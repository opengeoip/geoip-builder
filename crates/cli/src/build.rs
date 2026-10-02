use std::collections::{BTreeSet, HashMap};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Read};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use flate2::read::MultiGzDecoder;
use mmdb_writer::Writer;
use model::{AsName, Asn, Assignment, Delegation, GeofeedRef, Location, PrefixMap, Registry};
use src_bgp::RibCollector;
use src_rpki::Validator;

use src_geofeed::list;

use crate::sources;

fn open(dir: &Path, source: &fetch::Source) -> Result<Box<dyn BufRead>> {
    let path = source.path(dir);
    let file = File::open(&path)
        .with_context(|| format!("opening {}, run fetch first", path.display()))?;
    let reader = BufReader::with_capacity(1 << 20, file);
    Ok(if source.name.ends_with(".gz") {
        Box::new(BufReader::with_capacity(
            1 << 20,
            MultiGzDecoder::new(reader),
        ))
    } else {
        Box::new(reader)
    })
}

fn write(writer: Writer, path: &Path) -> Result<()> {
    let partial = path.with_extension("mmdb.part");
    writer.write_to(BufWriter::new(File::create(&partial)?))?;
    fs::rename(&partial, path)?;
    eprintln!(
        "wrote {} ({} bytes)",
        path.display(),
        fs::metadata(path)?.len()
    );
    Ok(())
}

pub fn geofeed_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("geofeeds")
}

pub struct RpslRecords {
    pub references: Vec<(Registry, GeofeedRef)>,
    pub assignments: Vec<Assignment>,
}

pub fn rpsl_records(data_dir: &Path) -> Result<RpslRecords> {
    let mut references = Vec::new();
    let mut assignments = Vec::new();
    for (source, registry) in sources::rpsl() {
        let parsed = src_rpsl::parse(open(data_dir, &source)?, registry)
            .with_context(|| format!("parsing {}", source.name))?;
        let stats = &parsed.stats;
        eprintln!(
            "{}: {} objects, {} with a country, {} geofeed references, {} rejected",
            source.name, stats.objects, stats.countries, stats.references, stats.rejected
        );
        references.extend(parsed.references.into_iter().map(|r| (registry, r)));
        assignments.extend(parsed.assignments);
    }
    let source = sources::arin_geofeed_inetnums();
    let (_, arin, stats) = src_arin::parse(open(data_dir, &source)?)
        .with_context(|| format!("parsing {}", source.name))?;
    eprintln!(
        "{}: {} objects, {} geofeed references, {} rejected",
        source.name, stats.objects, stats.references, stats.rejected
    );
    references.extend(arin.into_iter().map(|r| (Registry::Arin, r)));
    Ok(RpslRecords {
        references,
        assignments,
    })
}

pub fn discover(data_dir: &Path, geofeeds: &Path) -> Result<()> {
    let references = rpsl_records(data_dir)?.references;
    let discovered = references
        .into_iter()
        .map(|(registry, reference)| list::Row {
            url: reference.url,
            network: Some(reference.network),
            source: registry.as_str().to_string(),
        });
    let rows = list::merge_discovered(geofeeds, discovered)?;
    eprintln!("{}: {rows} rows", geofeeds.display());
    Ok(())
}

pub fn geofeed_urls(rows: &[list::Row]) -> Vec<String> {
    let urls: BTreeSet<&str> = rows.iter().map(|r| r.url.as_str()).collect();
    urls.into_iter().map(str::to_string).collect()
}

fn read_feed(dir: &Path, url: &str) -> Option<Vec<Location>> {
    let file = File::open(dir.join(src_geofeed::cache_name(url))).ok()?;
    let reader = BufReader::new(file.take(1 << 30));
    src_geofeed::parse(reader)
        .ok()
        .map(|(locations, _)| locations)
}

pub struct Inputs<'a> {
    pub data_dir: &'a Path,
    pub collectors: &'a [String],
    pub vrps_url: &'a str,
    pub policy: merge::RpkiPolicy,
    pub geofeeds: &'a Path,
}

pub struct Prepared {
    pub epoch: u64,
    pub policy: merge::RpkiPolicy,
    pub names: HashMap<Asn, AsName>,
    pub selected: Vec<merge::SelectedRoute>,
    pub delegations: Vec<Delegation>,
    pub assignments: Vec<Assignment>,
    pub listed: Vec<Location>,
    pub anchored: Vec<Location>,
}

pub fn prepare(inputs: &Inputs<'_>) -> Result<Prepared> {
    let data_dir = inputs.data_dir;
    let epoch = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

    let started = Instant::now();
    let names = src_asnames::parse(open(data_dir, &sources::asnames())?)?;
    eprintln!("asn.txt: {} names", names.len());
    let vrps = src_rpki::parse(open(data_dir, &sources::vrps(inputs.vrps_url))?)?;
    eprintln!("vrps.json: {} VRPs", vrps.len());
    let validator = Validator::new(&vrps);
    drop(vrps);
    let mut rib = RibCollector::default();
    for collector in inputs.collectors {
        let source = sources::ris(collector);
        rib.add_file(&source.path(data_dir))?;
        eprintln!("{}: parsed after {:.1?}", source.name, started.elapsed());
    }
    let routes = rib.into_routes();
    let (selected, stats) = merge::select_origins(&routes, &validator, inputs.policy);
    drop(routes);
    eprintln!(
        "asn: {} routes, {} valid, {} invalid, {} not found, in {:.1?}",
        stats.routes,
        stats.valid,
        stats.invalid,
        stats.not_found,
        started.elapsed()
    );
    let origins: PrefixMap<Asn> = selected.iter().map(|r| (r.prefix, r.asn)).collect();

    let mut delegations = Vec::new();
    for registry in Registry::ALL {
        let parsed = src_delegated::parse(open(data_dir, &sources::delegated(registry))?)
            .with_context(|| format!("parsing delegated-{registry}"))?;
        eprintln!("delegated-{registry}: {} networks", parsed.len());
        delegations.extend(parsed);
    }

    let assignments = rpsl_records(data_dir)?.assignments;
    let rows = list::read(inputs.geofeeds)?;
    let references: Vec<GeofeedRef> = rows
        .iter()
        .filter_map(|row| {
            row.network.map(|network| GeofeedRef {
                network,
                url: row.url.clone(),
            })
        })
        .collect();
    let listed_urls: BTreeSet<&str> = rows
        .iter()
        .filter(|row| row.network.is_none())
        .map(|row| row.url.as_str())
        .collect();
    eprintln!(
        "{}: {} anchored references, {} unanchored geofeeds",
        inputs.geofeeds.display(),
        references.len(),
        listed_urls.len()
    );
    let dir = geofeed_dir(data_dir);
    let mut feeds: HashMap<String, Vec<Location>> = HashMap::new();
    let mut missing = 0;
    for url in references.iter().map(|r| &r.url).collect::<BTreeSet<_>>() {
        match read_feed(&dir, url) {
            Some(locations) => {
                feeds.insert(url.clone(), locations);
            }
            None => missing += 1,
        }
    }
    eprintln!(
        "geofeeds: {} referenced feeds read, {} missing",
        feeds.len(),
        missing
    );
    let (anchored, stats) = merge::authorize_geofeeds(&references, &feeds);
    eprintln!(
        "geofeeds: {} entries, {} accepted, {} without country, {} outside their inetnum, {} overridden by a more specific inetnum",
        stats.entries, stats.accepted, stats.no_country, stats.not_anchored, stats.overridden
    );
    drop(feeds);

    let listed: Vec<(String, Vec<Location>)> = listed_urls
        .into_iter()
        .filter_map(|url| read_feed(&dir, url).map(|locations| (url.to_string(), locations)))
        .collect();
    let (listed, reports) = merge::authorize_listed_geofeeds(&listed, &origins, &names);
    for report in &reports {
        let publishers: Vec<String> = report
            .publishers
            .iter()
            .map(|asn| format!("AS{asn}"))
            .collect();
        eprintln!(
            "listed geofeed {}: {} entries, {} accepted, {} unrouted, publisher {}",
            report.url,
            report.entries,
            report.accepted,
            report.unrouted,
            publishers.join(" ")
        );
    }

    Ok(Prepared {
        epoch,
        policy: inputs.policy,
        names,
        selected,
        delegations,
        assignments,
        listed,
        anchored,
    })
}

pub fn run(inputs: &Inputs<'_>, out_dir: &Path) -> Result<()> {
    fs::create_dir_all(out_dir)?;
    let prepared = prepare(inputs)?;
    let (asn, unnamed) = merge::asn_db(
        &prepared.selected,
        &prepared.names,
        prepared.policy,
        prepared.epoch,
    );
    eprintln!("asn: {unnamed} routes kept without a name");
    write(asn, &out_dir.join("asn.mmdb"))?;

    let started = Instant::now();
    let (country, city, stats) = merge::location_dbs(
        prepared.delegations,
        prepared.assignments,
        vec![prepared.listed, prepared.anchored],
        prepared.epoch,
    );
    eprintln!(
        "location: {} delegations, {} inetnum countries applied, {} ignored (not inside a delegation of their registry), {} geofeed entries, in {:.1?}",
        stats.delegations,
        stats.assignments,
        stats.ignored_assignments,
        stats.geofeed_entries,
        started.elapsed()
    );
    write(country, &out_dir.join("country.mmdb"))?;
    write(city, &out_dir.join("city.mmdb"))?;
    Ok(())
}
