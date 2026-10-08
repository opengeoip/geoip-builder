use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use model::{Asn, Assignment, GeofeedRef, Location, Registry};
use src_geofeed::list;

use crate::io::open;
use crate::{Log, arin_check, sources};

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

pub fn is_remote(spec: &str) -> bool {
    spec.starts_with("https://") || spec.starts_with("http://")
}

pub fn path(data_dir: &Path, spec: &str) -> PathBuf {
    if is_remote(spec) {
        sources::catalog(spec).path(data_dir)
    } else {
        PathBuf::from(spec)
    }
}

fn discovered(data_dir: &Path, log: &mut Log<'_>) -> Result<Vec<list::Row>> {
    Ok(rpsl_records(data_dir, log)?
        .references
        .into_iter()
        .map(|(registry, reference)| list::Row {
            url: reference.url,
            network: Some(reference.network),
            source: registry.as_str().to_string(),
            asn: Vec::new(),
        })
        .collect())
}

pub fn refresh(data_dir: &Path, catalog: &Path, log: &mut Log<'_>) -> Result<()> {
    let rows = list::merge_discovered(catalog, discovered(data_dir, log)?)?;
    log(format!("{}: {rows} rows", catalog.display()));
    Ok(())
}

pub struct DiscoverOptions<'a> {
    pub data_dir: &'a Path,
    pub output: &'a Path,
    pub manual: Option<&'a Path>,
    pub fetch: bool,
    pub arin_check_sample: usize,
}

pub fn discover(options: &DiscoverOptions<'_>, log: &mut Log<'_>) -> Result<()> {
    if options.fetch {
        let fetcher = ::fetch::Fetcher::new(options.data_dir)?;
        for source in sources::registries() {
            fetcher
                .fetch(&source)
                .with_context(|| format!("downloading {}", source.name))?;
            log(format!("{}: up to date", source.name));
        }
    }
    let check = arin_check::run(
        options.data_dir,
        options.arin_check_sample,
        Duration::from_secs(1),
        log,
    )?;
    if check.differing > 0 {
        anyhow::bail!(
            "{} sampled ARIN records differ from RDAP, refusing the ARIN geofeed references",
            check.differing
        );
    }
    let Some(manual) = options.manual else {
        return refresh(options.data_dir, options.output, log);
    };
    let manual = list::read_manual(manual)?;
    let problems = list::problems(&manual);
    if !problems.is_empty() {
        anyhow::bail!("invalid manual rows: {}", problems.join("; "));
    }
    let rows = list::write(
        options.output,
        manual.into_iter().chain(discovered(options.data_dir, log)?),
    )?;
    log(format!("{}: {rows} rows", options.output.display()));
    Ok(())
}

pub struct CheckReport {
    pub rows: usize,
    pub urls: usize,
    pub anchored: usize,
}

pub fn check(catalog: &Path, manual: bool) -> Result<CheckReport> {
    let rows = if manual {
        list::read_manual(catalog)?
    } else {
        list::read(catalog)?
    };
    let problems = list::problems(&rows);
    if !problems.is_empty() {
        anyhow::bail!("{}: {}", catalog.display(), problems.join("; "));
    }
    Ok(CheckReport {
        rows: rows.len(),
        urls: geofeed_urls(&rows).len(),
        anchored: rows.iter().filter(|r| r.network.is_some()).count(),
    })
}

pub fn geofeed_urls(rows: &[list::Row]) -> Vec<String> {
    let urls: BTreeSet<&str> = rows.iter().map(|r| r.url.as_str()).collect();
    urls.into_iter().map(str::to_string).collect()
}

pub struct Catalog {
    pub references: Vec<(Registry, GeofeedRef)>,
    pub unanchored: BTreeMap<String, BTreeSet<Asn>>,
}

pub fn catalog(geofeeds: &Path, log: &mut Log<'_>) -> Result<Catalog> {
    let mut references = Vec::new();
    let mut ignored = 0;
    let mut unanchored: BTreeMap<String, BTreeSet<Asn>> = BTreeMap::new();
    for row in list::read(geofeeds)? {
        match (row.network, row.source.parse::<Registry>()) {
            (Some(network), Ok(registry)) => references.push((
                registry,
                GeofeedRef {
                    network,
                    url: row.url,
                },
            )),
            (Some(_), Err(_)) => ignored += 1,
            (None, _) => unanchored.entry(row.url).or_default().extend(&row.asn),
        }
    }
    log(format!(
        "{}: {} anchored references, {} unanchored geofeeds, {ignored} anchored rows not from a registry ignored",
        geofeeds.display(),
        references.len(),
        unanchored.len(),
    ));
    Ok(Catalog {
        references,
        unanchored,
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
