use std::collections::{BTreeSet, HashMap};
use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;
use std::thread;
use std::time::Duration;

use analysis::candidates::{Candidate, guesses, looks_like_geofeed, normalized, website_host};
use anyhow::{Context, Result};
use fetch::Fetcher;
use maxminddb::Reader;
use serde::Deserialize;
use src_geofeed::list;

use crate::fetch::geofeed_fetcher;
use crate::{sources, truth};

pub struct Options<'a> {
    pub data_dir: &'a Path,
    pub truth: &'a Path,
    pub country: &'a Path,
    pub asn: &'a Path,
    pub coverage: &'a Path,
    pub geofeeds: &'a Path,
    pub probe: usize,
}

#[derive(Deserialize)]
struct PeeringDb {
    data: Vec<Network>,
}

#[derive(Clone, Deserialize)]
pub struct Network {
    pub asn: u32,
    pub name: String,
    #[serde(default)]
    pub website: Option<String>,
    #[serde(default)]
    pub info_type: Option<String>,
}

#[derive(Deserialize)]
pub struct CoverageRow {
    pub asn: u32,
    pub name: String,
    pub ipv4_share_without_geofeed: f64,
}

pub struct Row {
    pub asn: u32,
    pub name: String,
    pub peeringdb_type: String,
    pub website: String,
    pub candidate: Candidate,
    pub ipv4_share_without_geofeed: Option<f64>,
    pub found_geofeed: String,
}

pub struct Report {
    pub coverage_missing: bool,
    pub rows: Vec<Row>,
    pub new_geofeeds: Vec<(u32, String)>,
}

pub fn networks(data_dir: &Path) -> Result<HashMap<u32, Network>> {
    match File::open(sources::peeringdb().path(data_dir)) {
        Ok(file) => Ok(
            serde_json::from_reader::<_, PeeringDb>(BufReader::new(file))?
                .data
                .into_iter()
                .map(|n| (n.asn, n))
                .collect(),
        ),
        Err(_) => Ok(HashMap::new()),
    }
}

pub fn coverage_rows(path: &Path) -> Result<Option<HashMap<u32, CoverageRow>>> {
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(
        csv::Reader::from_path(path)?
            .deserialize::<CoverageRow>()
            .filter_map(Result::ok)
            .map(|row| (row.asn, row))
            .collect(),
    ))
}

fn is_geofeed(path: &Path) -> bool {
    File::open(path).is_ok_and(|file| looks_like_geofeed(BufReader::new(file)))
}

pub fn find_geofeed(fetcher: &Fetcher, host: &str) -> Option<String> {
    for url in guesses(host) {
        let source = src_geofeed::source(&url);
        let path = source.path(fetcher.dir());
        let existed = path.exists();
        let ok = fetcher.fetch(&source).is_ok() && is_geofeed(&path);
        thread::sleep(Duration::from_millis(300));
        if ok {
            return Some(url);
        }
        if !existed {
            let _ = fs::remove_file(&path);
            let _ = fs::remove_file(fetcher.dir().join(format!("{}.meta.json", source.name)));
        }
    }
    None
}

pub fn candidates(options: &Options<'_>) -> Result<Report> {
    let probes = src_atlas::parse(BufReader::new(File::open(options.truth)?))?;
    let (probes, _) = truth::reliable(options.data_dir, probes)?;
    let country = Reader::open_readfile(options.country)
        .with_context(|| format!("opening {}, run build first", options.country.display()))?;
    let asn = Reader::open_readfile(options.asn)?;
    let ranked =
        analysis::candidates::rank(analysis::candidates::observe(&probes, &country, &asn)?);

    let coverage = coverage_rows(options.coverage)?;
    let coverage_missing = coverage.is_none();
    let coverage = coverage.unwrap_or_default();
    let networks = networks(options.data_dir)?;
    let fetcher = geofeed_fetcher(options.data_dir, Duration::from_secs(60))?;
    let known: BTreeSet<String> = list::read(options.geofeeds)?
        .iter()
        .map(|r| normalized(&r.url))
        .collect();

    let mut rows = Vec::new();
    let mut new_geofeeds = Vec::new();
    for (index, (asn_number, candidate)) in ranked.into_iter().enumerate() {
        let network = networks.get(&asn_number);
        let website = network.and_then(|n| n.website.clone()).unwrap_or_default();
        let mut found_geofeed = String::new();
        if index < options.probe
            && let Some(host) = website_host(&website)
            && let Some(url) = find_geofeed(&fetcher, &host)
        {
            if !known.contains(&normalized(&url)) {
                new_geofeeds.push((asn_number, url.clone()));
            }
            found_geofeed = url;
        }
        rows.push(Row {
            asn: asn_number,
            name: network
                .map(|n| n.name.clone())
                .or_else(|| coverage.get(&asn_number).map(|c| c.name.clone()))
                .unwrap_or_default(),
            peeringdb_type: network
                .and_then(|n| n.info_type.clone())
                .unwrap_or_default(),
            website,
            candidate,
            ipv4_share_without_geofeed: coverage
                .get(&asn_number)
                .map(|c| c.ipv4_share_without_geofeed),
            found_geofeed,
        });
    }
    Ok(Report {
        coverage_missing,
        rows,
        new_geofeeds,
    })
}

pub fn add_to_manual(
    manual: &Path,
    found: impl IntoIterator<Item = (u32, String)>,
) -> Result<usize> {
    let mut rows = list::read_manual(manual)?;
    rows.extend(found.into_iter().map(|(asn, url)| list::Row {
        url,
        network: None,
        source: list::MANUAL.to_string(),
        asn: vec![asn],
    }));
    list::write_manual(manual, rows)
}
