use std::collections::{BTreeSet, HashMap};
use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use fetch::Fetcher;
use maxminddb::{Reader, path};
use serde::Deserialize;
use src_geofeed::list;

use crate::{build, evaluate, sources, truth};

pub struct Options<'a> {
    pub data_dir: &'a Path,
    pub truth: &'a Path,
    pub country: &'a Path,
    pub asn: &'a Path,
    pub coverage: &'a Path,
    pub geofeeds: &'a Path,
    pub output: &'a Path,
    pub probe: usize,
    pub add: bool,
}

#[derive(Deserialize)]
struct PeeringDb {
    data: Vec<Network>,
}

#[derive(Clone, Deserialize)]
struct Network {
    asn: u32,
    name: String,
    #[serde(default)]
    website: Option<String>,
    #[serde(default)]
    info_type: Option<String>,
}

#[derive(Deserialize)]
struct CoverageRow {
    asn: u32,
    name: String,
    ipv4_share_without_geofeed: f64,
}

#[derive(Default)]
struct Candidate {
    probes: usize,
    wrong: usize,
    errors: HashMap<(String, String), usize>,
}

fn host(website: &str) -> Option<String> {
    let rest = website.split_once("://").map_or(website, |(_, rest)| rest);
    let host = rest
        .split(['/', ':', '?', '#'])
        .next()?
        .trim()
        .to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host).to_string();
    (host.contains('.') && !host.is_empty()).then_some(host)
}

fn guesses(host: &str) -> Vec<String> {
    let mut urls: Vec<String> = [
        "geofeed.csv",
        "geofeed",
        "geofeed.txt",
        ".well-known/geofeed",
        ".well-known/geofeed.csv",
    ]
    .iter()
    .flat_map(|path| {
        [
            format!("https://{host}/{path}"),
            format!("https://www.{host}/{path}"),
        ]
    })
    .collect();
    urls.push(format!("https://geofeed.{host}/"));
    urls.push(format!("https://geofeed.{host}/geofeed.csv"));
    urls
}

fn normalized(url: &str) -> String {
    let lower = url.to_ascii_lowercase();
    let rest = lower
        .split_once("://")
        .map_or(lower.as_str(), |(_, rest)| rest);
    rest.strip_prefix("www.")
        .unwrap_or(rest)
        .trim_end_matches('/')
        .to_string()
}

fn looks_like_geofeed(path: &Path) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };
    match src_geofeed::parse(BufReader::new(file)) {
        Ok((locations, stats)) => locations.len() >= 10 && stats.invalid * 4 <= stats.entries,
        Err(_) => false,
    }
}

fn discover_feed(fetcher: &Fetcher, host: &str) -> Option<String> {
    for url in guesses(host) {
        let source = src_geofeed::source(&url);
        let path = source.path(fetcher.dir());
        let existed = path.exists();
        let ok = fetcher.fetch(&source).is_ok() && looks_like_geofeed(&path);
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

pub fn run(options: &Options<'_>) -> Result<()> {
    let probes = src_atlas::parse(BufReader::new(File::open(options.truth)?))?;
    let (probes, _) = truth::reliable(options.data_dir, probes)?;
    let country = Reader::open_readfile(options.country)
        .with_context(|| format!("opening {}, run build first", options.country.display()))?;
    let asn = Reader::open_readfile(options.asn)?;

    let mut candidates: HashMap<u32, Candidate> = HashMap::new();
    for probe in &probes {
        let Some(origin) = asn
            .lookup(probe.address)?
            .decode_path::<u32>(&path!["autonomous_system_number"])?
        else {
            continue;
        };
        let answer = evaluate::country(&country, probe)?;
        let candidate = candidates.entry(origin).or_default();
        candidate.probes += 1;
        if answer.as_deref() != Some(probe.country.as_str()) {
            candidate.wrong += 1;
            *candidate
                .errors
                .entry((probe.country.clone(), answer.unwrap_or_else(|| "-".into())))
                .or_default() += 1;
        }
    }

    let coverage: HashMap<u32, CoverageRow> = if options.coverage.exists() {
        csv::Reader::from_path(options.coverage)?
            .deserialize::<CoverageRow>()
            .filter_map(Result::ok)
            .map(|row| (row.asn, row))
            .collect()
    } else {
        eprintln!(
            "{} not found, run coverage for the share without geofeed",
            options.coverage.display()
        );
        HashMap::new()
    };
    let peeringdb = sources::peeringdb().path(options.data_dir);
    let networks: HashMap<u32, Network> = match File::open(&peeringdb) {
        Ok(file) => serde_json::from_reader::<_, PeeringDb>(BufReader::new(file))?
            .data
            .into_iter()
            .map(|n| (n.asn, n))
            .collect(),
        Err(_) => HashMap::new(),
    };

    let mut ranked: Vec<(u32, Candidate)> = candidates
        .into_iter()
        .filter(|(_, c)| c.wrong > 0)
        .collect();
    ranked.sort_by(|a, b| b.1.wrong.cmp(&a.1.wrong).then(a.0.cmp(&b.0)));

    let fetcher = Fetcher::with_options(
        build::geofeed_dir(options.data_dir),
        fetch::Options {
            connect_timeout: Duration::from_secs(10),
            global_timeout: Some(Duration::from_secs(60)),
            max_size: 256 << 20,
            verify_tls: false,
        },
    )?;
    let known: BTreeSet<String> = list::read(options.geofeeds)?
        .iter()
        .map(|r| normalized(&r.url))
        .collect();
    let mut found: Vec<(u32, String)> = Vec::new();

    if let Some(parent) = options.output.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = csv::Writer::from_path(options.output)?;
    out.write_record([
        "asn",
        "name",
        "peeringdb_type",
        "website",
        "wrong_probes",
        "probes",
        "top_error",
        "ipv4_share_without_geofeed",
        "found_geofeed",
    ])?;
    for (index, (asn_number, candidate)) in ranked.iter().enumerate() {
        let network = networks.get(asn_number);
        let website = network.and_then(|n| n.website.clone()).unwrap_or_default();
        let mut feed = String::new();
        if index < options.probe
            && let Some(host) = host(&website)
            && let Some(url) = discover_feed(&fetcher, &host)
        {
            if !known.contains(&normalized(&url)) {
                found.push((*asn_number, url.clone()));
            }
            feed = url;
        }
        let name = network
            .map(|n| n.name.clone())
            .or_else(|| coverage.get(asn_number).map(|c| c.name.clone()))
            .unwrap_or_default();
        let top_error = candidate
            .errors
            .iter()
            .max_by_key(|(pair, count)| (**count, std::cmp::Reverse((*pair).clone())))
            .map(|((truth, ours), count)| format!("{truth}->{ours} x{count}"))
            .unwrap_or_default();
        out.write_record([
            asn_number.to_string(),
            name,
            network
                .and_then(|n| n.info_type.clone())
                .unwrap_or_default(),
            website,
            candidate.wrong.to_string(),
            candidate.probes.to_string(),
            top_error,
            coverage
                .get(asn_number)
                .map(|c| format!("{:.4}", c.ipv4_share_without_geofeed))
                .unwrap_or_default(),
            feed,
        ])?;
    }
    out.flush()?;

    eprintln!(
        "candidates: {} ASes with misplaced probes, written to {}",
        ranked.len(),
        options.output.display()
    );
    for (asn_number, url) in &found {
        eprintln!("  AS{asn_number}: new geofeed {url}");
    }
    if options.add && !found.is_empty() {
        let mut rows = list::read(options.geofeeds)?;
        rows.extend(found.into_iter().map(|(_, url)| list::Row {
            url,
            network: None,
            source: list::MANUAL.to_string(),
        }));
        let count = list::write(options.geofeeds, rows)?;
        eprintln!("{}: {count} rows", options.geofeeds.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_hosts_from_websites() {
        assert_eq!(
            host("http://www.m247global.com").as_deref(),
            Some("m247global.com")
        );
        assert_eq!(
            host("https://cloud.oracle.com/").as_deref(),
            Some("cloud.oracle.com")
        );
        assert_eq!(host("example").as_deref(), None);
        assert_eq!(host("").as_deref(), None);
    }

    #[test]
    fn compares_urls_regardless_of_scheme_and_www() {
        assert_eq!(
            normalized("https://www.Hetzner.com/geofeed.csv"),
            normalized("http://hetzner.com/geofeed.csv")
        );
        assert_ne!(
            normalized("https://a.example/x"),
            normalized("https://b.example/x")
        );
    }

    #[test]
    fn guesses_common_geofeed_locations() {
        let urls = guesses("example.net");
        assert!(urls.contains(&"https://example.net/geofeed.csv".to_string()));
        assert!(urls.contains(&"https://geofeed.example.net/".to_string()));
    }
}
