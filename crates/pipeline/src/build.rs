use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use model::{AsName, Asn, Assignment, Delegation, GeofeedRef, Location, PrefixMap, Registry};
use src_bgp::RibCollector;
use src_rpki::Validator;

use crate::catalog::{self, geofeed_dir, read_feed, read_feeds};
use crate::io::{open, write};
use crate::{Log, sources};

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

pub fn names(data_dir: &Path, log: &mut Log<'_>) -> Result<HashMap<Asn, AsName>> {
    let names = src_asnames::parse(open(data_dir, &sources::asnames())?)?;
    log(format!("asn.txt: {} names", names.len()));
    Ok(names)
}

pub fn routes(
    inputs: &Inputs<'_>,
    started: Instant,
    log: &mut Log<'_>,
) -> Result<Vec<merge::SelectedRoute>> {
    let vrps = src_rpki::parse(open(inputs.data_dir, &sources::vrps(inputs.vrps_url))?)?;
    log(format!("vrps.json: {} VRPs", vrps.len()));
    let validator = Validator::new(&vrps);
    drop(vrps);
    let mut rib = RibCollector::default();
    for collector in inputs.collectors {
        let source = sources::ris(collector);
        rib.add_file(&source.path(inputs.data_dir))?;
        log(format!(
            "{}: parsed after {:.1?}",
            source.name,
            started.elapsed()
        ));
    }
    let routes = rib.into_routes();
    let (selected, stats) = merge::select_origins(&routes, &validator, inputs.policy);
    log(format!(
        "asn: {} routes, {} valid, {} invalid, {} not found, in {:.1?}",
        stats.routes,
        stats.valid,
        stats.invalid,
        stats.not_found,
        started.elapsed()
    ));
    Ok(selected)
}

pub fn delegations(data_dir: &Path, log: &mut Log<'_>) -> Result<Vec<Delegation>> {
    let mut delegations = Vec::new();
    for registry in Registry::ALL {
        let parsed = src_delegated::parse(open(data_dir, &sources::delegated(registry))?)
            .with_context(|| format!("parsing delegated-{registry}"))?;
        log(format!("delegated-{registry}: {} networks", parsed.len()));
        delegations.extend(parsed);
    }
    Ok(delegations)
}

pub fn anchored_geofeeds(
    data_dir: &Path,
    references: &[GeofeedRef],
    log: &mut Log<'_>,
) -> Vec<Location> {
    let urls: BTreeSet<&str> = references.iter().map(|r| r.url.as_str()).collect();
    let (feeds, missing) = read_feeds(&geofeed_dir(data_dir), urls);
    log(format!(
        "geofeeds: {} referenced feeds read, {} missing",
        feeds.len(),
        missing
    ));
    let (anchored, stats) = merge::authorize_geofeeds(references, &feeds);
    log(format!(
        "geofeeds: {} entries, {} accepted, {} without country, {} outside their inetnum, {} overridden by a more specific inetnum",
        stats.entries, stats.accepted, stats.no_country, stats.not_anchored, stats.overridden
    ));
    anchored
}

pub fn listed_geofeeds(
    data_dir: &Path,
    unanchored: &BTreeMap<String, BTreeSet<Asn>>,
    selected: &[merge::SelectedRoute],
    log: &mut Log<'_>,
) -> Vec<Location> {
    let dir = geofeed_dir(data_dir);
    let feeds: Vec<merge::ListedGeofeed> = unanchored
        .iter()
        .filter_map(|(url, asns)| {
            read_feed(&dir, url).map(|locations| merge::ListedGeofeed {
                url: url.clone(),
                asns: asns.iter().copied().collect(),
                locations,
            })
        })
        .collect();
    let origins: PrefixMap<Asn> = selected.iter().map(|r| (r.prefix, r.asn)).collect();
    let (listed, reports) = merge::authorize_listed_geofeeds(&feeds, &origins);
    for report in &reports {
        let undeclared: Vec<String> = report
            .undeclared
            .iter()
            .take(5)
            .map(|(asn, count)| format!("AS{asn} ({count})"))
            .collect();
        log(format!(
            "listed geofeed {}: {} entries, {} accepted, {} unrouted, undeclared origins: {}",
            report.url,
            report.entries,
            report.accepted,
            report.unrouted,
            if undeclared.is_empty() {
                "none".to_string()
            } else {
                undeclared.join(" ")
            }
        ));
    }
    listed
}

pub fn prepare(inputs: &Inputs<'_>, log: &mut Log<'_>) -> Result<Prepared> {
    let epoch = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let started = Instant::now();
    let names = names(inputs.data_dir, log)?;
    let selected = routes(inputs, started, log)?;
    let delegations = delegations(inputs.data_dir, log)?;
    let assignments = catalog::rpsl_records(inputs.data_dir, log)?.assignments;
    let catalog = catalog::catalog(inputs.geofeeds, log)?;
    let anchored = anchored_geofeeds(inputs.data_dir, &catalog.references, log);
    let listed = listed_geofeeds(inputs.data_dir, &catalog.unanchored, &selected, log);
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

fn write_logged(writer: mmdb_writer::Writer, path: &Path, log: &mut Log<'_>) -> Result<()> {
    let size = write(writer, path)?;
    log(format!("wrote {} ({size} bytes)", path.display()));
    Ok(())
}

pub fn build(inputs: &Inputs<'_>, out_dir: &Path, log: &mut Log<'_>) -> Result<()> {
    fs::create_dir_all(out_dir)?;
    let prepared = prepare(inputs, log)?;
    let (asn, unnamed) = merge::asn_db(
        &prepared.selected,
        &prepared.names,
        prepared.policy,
        prepared.epoch,
    );
    log(format!("asn: {unnamed} routes kept without a name"));
    write_logged(asn, &out_dir.join("asn.mmdb"), log)?;

    let started = Instant::now();
    let (country, city, stats) = merge::location_dbs(
        prepared.delegations,
        prepared.assignments,
        vec![prepared.listed, prepared.anchored],
        prepared.epoch,
    );
    log(format!(
        "location: {} delegations, {} inetnum countries applied, {} ignored (not inside a delegation of their registry), {} geofeed entries, in {:.1?}",
        stats.delegations,
        stats.assignments,
        stats.ignored_assignments,
        stats.geofeed_entries,
        started.elapsed()
    ));
    write_logged(country, &out_dir.join("country.mmdb"), log)?;
    write_logged(city, &out_dir.join("city.mmdb"), log)?;
    Ok(())
}
