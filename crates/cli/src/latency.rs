use std::collections::HashMap;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use fetch::{Fetcher, Source};
use ipnet::IpNet;
use merge::PrefixPings;
use src_atlas::{Measurement, MeasurementPage};

use crate::{sources, truth};

const API: &str = "https://atlas.ripe.net/api/v2/measurements";

fn dir(data_dir: &Path) -> PathBuf {
    data_dir.join("latency")
}

fn index_path(data_dir: &Path) -> PathBuf {
    dir(data_dir).join(format!("{}-index.json", sources::WHEREIS_TAG))
}

fn results_dir(data_dir: &Path) -> PathBuf {
    dir(data_dir).join(sources::WHEREIS_TAG)
}

fn fetch_index(data_dir: &Path, interval: Duration) -> Result<Vec<Measurement>> {
    let path = index_path(data_dir);
    if path.exists() {
        return Ok(serde_json::from_reader(BufReader::new(File::open(&path)?))?);
    }
    let fetcher = Fetcher::new(dir(data_dir).join("pages"))?;
    let mut url = format!(
        "{API}/?tags={}&page_size=500&fields=id,description,target",
        sources::WHEREIS_TAG
    );
    let mut measurements = Vec::new();
    for page in 0.. {
        let source = Source::new(format!("page-{page}.json"), url.clone());
        fetcher.fetch(&source)?;
        let parsed: MeasurementPage =
            serde_json::from_reader(BufReader::new(File::open(source.path(fetcher.dir()))?))?;
        measurements.extend(parsed.results);
        match parsed.next {
            Some(next) => url = next,
            None => break,
        }
        thread::sleep(interval);
    }
    fs::write(&path, serde_json::to_vec(&measurements)?)?;
    fs::remove_dir_all(fetcher.dir())?;
    Ok(measurements)
}

pub fn run_fetch(data_dir: &Path, rate: f64, workers: usize, max: Option<usize>) -> Result<()> {
    let interval = Duration::from_secs_f64(1.0 / rate.max(0.1));
    let measurements = fetch_index(data_dir, interval)?;
    let results = results_dir(data_dir);
    let fetcher = Fetcher::new(&results)?;
    let missing: Vec<&Measurement> = measurements
        .iter()
        .filter(|m| !results.join(format!("{}.json", m.id)).exists())
        .collect();
    eprintln!(
        "{}: {} measurements, {} already downloaded, {} to fetch",
        sources::WHEREIS_TAG,
        measurements.len(),
        measurements.len() - missing.len(),
        missing.len()
    );
    let queue = Mutex::new(missing.into_iter().take(max.unwrap_or(usize::MAX)));
    let fetched = AtomicUsize::new(0);
    let stop = AtomicBool::new(false);
    let pace = interval * workers.max(1) as u32;
    thread::scope(|scope| {
        for _ in 0..workers.max(1) {
            scope.spawn(|| {
                while !stop.load(Ordering::Relaxed) {
                    let Some(measurement) = queue.lock().unwrap().next() else {
                        break;
                    };
                    let source = Source::new(
                        format!("{}.json", measurement.id),
                        format!("{API}/{}/results/?format=json", measurement.id),
                    );
                    let mut result = fetcher.fetch(&source);
                    if let Err(error) = &result
                        && fetch::is_rate_limited(error)
                    {
                        eprintln!("rate limited, pausing one minute");
                        thread::sleep(Duration::from_secs(60));
                        result = fetcher.fetch(&source);
                    }
                    match result {
                        Ok(_) => {
                            let done = fetched.fetch_add(1, Ordering::Relaxed) + 1;
                            if done.is_multiple_of(1000) {
                                eprintln!("{done} results downloaded");
                            }
                        }
                        Err(error) => {
                            eprintln!("{}: {error:#}", source.name);
                            if fetch::is_rate_limited(&error) {
                                stop.store(true, Ordering::Relaxed);
                            }
                        }
                    }
                    thread::sleep(pace);
                }
            });
        }
    });
    let fetched = fetched.into_inner();
    eprintln!("{}: {fetched} results downloaded", sources::WHEREIS_TAG);
    Ok(())
}

pub fn measurements(data_dir: &Path) -> Result<Vec<PrefixPings>> {
    let index = index_path(data_dir);
    if !index.exists() {
        return Ok(Vec::new());
    }
    let measurements: Vec<Measurement> =
        serde_json::from_reader(BufReader::new(File::open(&index)?))?;
    let archive = sources::atlas().path(data_dir);
    let sites = src_atlas::parse_sites(BufReader::new(
        File::open(&archive).with_context(|| format!("opening {}", archive.display()))?,
    ))?;
    let violating = truth::violating(data_dir)?;
    let countries: HashMap<u32, String> = sites
        .into_iter()
        .filter(|site| !violating.contains(&site.id))
        .map(|site| (site.id, site.country))
        .collect();
    let results = results_dir(data_dir);
    let mut out = Vec::new();
    for measurement in measurements {
        let Some(prefix) = measurement
            .description
            .strip_prefix("neo-ip-")
            .and_then(|p| p.parse::<IpNet>().ok())
        else {
            continue;
        };
        let Ok(file) = File::open(results.join(format!("{}.json", measurement.id))) else {
            continue;
        };
        let Ok(pings) = src_atlas::parse_ping_results(BufReader::new(file)) else {
            continue;
        };
        out.push(PrefixPings {
            prefix: prefix.trunc(),
            pings: pings
                .into_iter()
                .filter_map(|(probe, rtt)| countries.get(&probe).map(|c| (c.clone(), rtt)))
                .collect(),
        });
    }
    Ok(out)
}
