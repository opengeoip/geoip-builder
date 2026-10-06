use std::collections::{BTreeSet, HashMap};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use model::{Assignment, GeofeedRef, Location, Registry};
use src_geofeed::list;

use crate::io::open;
use crate::{Log, sources};

pub fn geofeed_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("geofeeds")
}

pub struct RpslRecords {
    pub references: Vec<(Registry, GeofeedRef)>,
    pub assignments: Vec<Assignment>,
}

pub fn rpsl_records(data_dir: &Path, log: &mut Log<'_>) -> Result<RpslRecords> {
    let mut references = Vec::new();
    let mut assignments = Vec::new();
    for (source, registry) in sources::rpsl() {
        let parsed = src_rpsl::parse(open(data_dir, &source)?, registry)
            .with_context(|| format!("parsing {}", source.name))?;
        let stats = &parsed.stats;
        log(format!(
            "{}: {} objects, {} with a country, {} geofeed references, {} rejected",
            source.name, stats.objects, stats.countries, stats.references, stats.rejected
        ));
        references.extend(parsed.references.into_iter().map(|r| (registry, r)));
        assignments.extend(parsed.assignments);
    }
    let source = sources::arin_geofeed_inetnums();
    let (_, arin, stats) = src_arin::parse(open(data_dir, &source)?)
        .with_context(|| format!("parsing {}", source.name))?;
    log(format!(
        "{}: {} objects, {} geofeed references, {} rejected",
        source.name, stats.objects, stats.references, stats.rejected
    ));
    references.extend(arin.into_iter().map(|r| (Registry::Arin, r)));
    Ok(RpslRecords {
        references,
        assignments,
    })
}

pub fn discover(data_dir: &Path, geofeeds: &Path, log: &mut Log<'_>) -> Result<()> {
    let references = rpsl_records(data_dir, log)?.references;
    let discovered = references
        .into_iter()
        .map(|(registry, reference)| list::Row {
            url: reference.url,
            network: Some(reference.network),
            source: registry.as_str().to_string(),
        });
    let rows = list::merge_discovered(geofeeds, discovered)?;
    log(format!("{}: {rows} rows", geofeeds.display()));
    Ok(())
}

pub fn geofeed_urls(rows: &[list::Row]) -> Vec<String> {
    let urls: BTreeSet<&str> = rows.iter().map(|r| r.url.as_str()).collect();
    urls.into_iter().map(str::to_string).collect()
}

pub struct Catalog {
    pub references: Vec<GeofeedRef>,
    pub unanchored: Vec<String>,
}

pub fn catalog(geofeeds: &Path, log: &mut Log<'_>) -> Result<Catalog> {
    let rows = list::read(geofeeds)?;
    let references: Vec<GeofeedRef> = rows
        .iter()
        .filter_map(|row| {
            row.network.map(|network| GeofeedRef {
                network,
                url: row.url.clone(),
            })
        })
        .collect();
    let unanchored: BTreeSet<&str> = rows
        .iter()
        .filter(|row| row.network.is_none())
        .map(|row| row.url.as_str())
        .collect();
    log(format!(
        "{}: {} anchored references, {} unanchored geofeeds",
        geofeeds.display(),
        references.len(),
        unanchored.len()
    ));
    Ok(Catalog {
        references,
        unanchored: unanchored.into_iter().map(str::to_string).collect(),
    })
}

pub fn read_feed(dir: &Path, url: &str) -> Option<Vec<Location>> {
    let file = File::open(dir.join(src_geofeed::cache_name(url))).ok()?;
    let reader = BufReader::new(file.take(1 << 30));
    src_geofeed::parse(reader)
        .ok()
        .map(|(locations, _)| locations)
}

pub fn read_feeds<'a>(
    dir: &Path,
    urls: impl IntoIterator<Item = &'a str>,
) -> (HashMap<String, Vec<Location>>, usize) {
    let mut feeds = HashMap::new();
    let mut missing = 0;
    for url in urls {
        match read_feed(dir, url) {
            Some(locations) => {
                feeds.insert(url.to_string(), locations);
            }
            None => missing += 1,
        }
    }
    (feeds, missing)
}
