use std::collections::HashSet;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;

use anyhow::{Context, Result};
use fetch::Fetcher;
use ipnet::IpNet;
use model::PrefixMap;
use serde::Deserialize;
use src_atlas::Probe;

use crate::{Log, sources};

#[derive(Deserialize)]
struct Entry {
    name: String,
}

pub fn fetch_violating(fetcher: &Fetcher, log: &mut Log<'_>) -> Result<()> {
    let index = sources::violating_probes_index();
    let entries: Vec<Entry> =
        serde_json::from_reader(BufReader::new(File::open(index.path(fetcher.dir()))?))
            .with_context(|| format!("parsing {}", index.name))?;
    let latest = entries
        .iter()
        .map(|e| e.name.as_str())
        .filter(|name| name.starts_with("probe_ids_") && name.ends_with(".txt"))
        .max()
        .context("no probe list in the index")?;
    let source = sources::violating_probes(latest);
    fetcher.fetch(&source)?;
    log(format!("{}: {latest}", source.name));
    Ok(())
}

pub fn violating(data_dir: &Path) -> Result<HashSet<u32>> {
    let path = sources::violating_probes("").path(data_dir);
    if !path.exists() {
        return Ok(HashSet::new());
    }
    src_atlas::parse_ids(BufReader::new(File::open(path)?))
}

pub struct Excluded {
    pub listed: usize,
    pub misplaced: usize,
    pub anycast: usize,
    pub auto_located: usize,
}

pub fn reliable(data_dir: &Path, probes: Vec<Probe>) -> Result<(Vec<Probe>, Excluded)> {
    let violating = violating(data_dir)?;
    let anycast = anycast(data_dir)?;
    let mut excluded = Excluded {
        listed: violating.len(),
        misplaced: 0,
        anycast: 0,
        auto_located: 0,
    };
    let kept = probes
        .into_iter()
        .filter(|probe| {
            if violating.contains(&probe.id) {
                excluded.misplaced += 1;
                false
            } else if anycast.longest_match(IpNet::from(probe.address)).is_some() {
                excluded.anycast += 1;
                false
            } else if probe.auto_located {
                excluded.auto_located += 1;
                false
            } else {
                true
            }
        })
        .collect();
    Ok((kept, excluded))
}

#[derive(Deserialize)]
struct AnycastRow {
    prefix: IpNet,
}

pub fn anycast(data_dir: &Path) -> Result<PrefixMap<()>> {
    let mut map = PrefixMap::default();
    for version in [4, 6] {
        let path = sources::anycast(version).path(data_dir);
        if !path.exists() {
            continue;
        }
        let text = fs::read_to_string(&path)?;
        let mut reader = csv::Reader::from_reader(text.as_bytes());
        for row in reader.deserialize::<AnycastRow>() {
            map.insert(row?.prefix, ());
        }
    }
    Ok(map)
}
