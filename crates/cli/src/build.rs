use std::collections::{BTreeSet, HashMap};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Read};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use flate2::read::MultiGzDecoder;
use mmdb_writer::Writer;
use model::{Asn, Assignment, GeofeedRef, Location, PrefixMap, Registry};
use src_bgp::RibCollector;
use src_rpki::Validator;

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

pub fn rpsl_records(data_dir: &Path) -> Result<(Vec<GeofeedRef>, Vec<Assignment>)> {
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
        references.extend(parsed.references);
        assignments.extend(parsed.assignments);
    }
    Ok((references, assignments))
}

pub fn geofeed_urls(references: &[GeofeedRef]) -> Vec<String> {
    let urls: BTreeSet<&str> = references
        .iter()
        .map(|r| r.url.as_str())
        .chain(sources::SEEDS.iter().map(|s| s.url))
        .collect();
    urls.into_iter().map(str::to_string).collect()
}

fn read_feed(dir: &Path, url: &str) -> Option<Vec<Location>> {
    let file = File::open(dir.join(src_geofeed::cache_name(url))).ok()?;
    let reader = BufReader::new(file.take(1 << 30));
    src_geofeed::parse(reader)
        .ok()
        .map(|(locations, _)| locations)
}

pub fn run(
    data_dir: &Path,
    out_dir: &Path,
    collectors: &[String],
    vrps_url: &str,
    policy: merge::RpkiPolicy,
) -> Result<()> {
    fs::create_dir_all(out_dir)?;
    let epoch = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

    let started = Instant::now();
    let names = src_asnames::parse(open(data_dir, &sources::asnames())?)?;
    eprintln!("asn.txt: {} names", names.len());
    let vrps = src_rpki::parse(open(data_dir, &sources::vrps(vrps_url))?)?;
    eprintln!("vrps.json: {} VRPs", vrps.len());
    let validator = Validator::new(&vrps);
    drop(vrps);
    let mut rib = RibCollector::default();
    for collector in collectors {
        let source = sources::ris(collector);
        rib.add_file(&source.path(data_dir))?;
        eprintln!("{}: parsed after {:.1?}", source.name, started.elapsed());
    }
    let routes = rib.into_routes();
    let (selected, stats) = merge::select_origins(&routes, &validator, policy);
    drop(routes);
    let (asn, unnamed) = merge::asn_db(&selected, &names, policy, epoch);
    eprintln!(
        "asn: {} routes, {} valid, {} invalid, {} not found, {} kept without a name, in {:.1?}",
        stats.routes,
        stats.valid,
        stats.invalid,
        stats.not_found,
        unnamed,
        started.elapsed()
    );
    write(asn, &out_dir.join("asn.mmdb"))?;
    let origins: PrefixMap<Asn> = selected.iter().map(|r| (r.prefix, r.asn)).collect();
    drop(selected);

    let started = Instant::now();
    let mut delegations = Vec::new();
    for registry in Registry::ALL {
        let parsed = src_delegated::parse(open(data_dir, &sources::delegated(registry))?)
            .with_context(|| format!("parsing delegated-{registry}"))?;
        eprintln!("delegated-{registry}: {} networks", parsed.len());
        delegations.extend(parsed);
    }

    let (references, assignments) = rpsl_records(data_dir)?;
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
    let seeds: Vec<(&[Asn], Vec<Location>)> = sources::SEEDS
        .iter()
        .filter_map(|seed| read_feed(&dir, seed.url).map(|locations| (seed.asns, locations)))
        .collect();
    eprintln!(
        "geofeeds: {} referenced feeds read, {} missing, {} of {} seed feeds read",
        feeds.len(),
        missing,
        seeds.len(),
        sources::SEEDS.len()
    );
    let (locations, stats) = merge::authorize_geofeeds(&references, &feeds, &seeds, &origins);
    eprintln!(
        "geofeeds: {} entries, {} accepted, {} without country, {} outside their inetnum, {} overridden by a more specific inetnum, {} seed entries accepted, {} rejected by origin AS",
        stats.entries,
        stats.accepted,
        stats.no_country,
        stats.not_anchored,
        stats.overridden,
        stats.seed_accepted,
        stats.seed_rejected
    );
    drop(feeds);

    let (country, city, stats) = merge::location_dbs(delegations, assignments, locations, epoch);
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
