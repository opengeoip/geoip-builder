use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::Path;

use anyhow::Result;
use ipnetwork::IpNetwork;
use maxminddb::Reader;
use mmdb_writer::Writer;

use crate::build::{self, Inputs};
use crate::compare::{Kind, intervals, sweep};

#[derive(Default, Clone, Copy)]
struct Space {
    announced: f64,
    uncovered: f64,
}

fn reader(writer: Writer) -> Result<Reader<Vec<u8>>> {
    let mut bytes = Vec::new();
    writer.write_to(&mut bytes)?;
    Ok(Reader::from_source(bytes)?)
}

fn spaces(
    asn: &Reader<Vec<u8>>,
    located: &Reader<Vec<u8>>,
    scope: &str,
    unit: f64,
) -> Result<HashMap<u32, Space>> {
    let scope: IpNetwork = scope.parse()?;
    let routed = intervals(asn, Kind::Asn, scope)?;
    let covered = intervals(located, Kind::Country, scope)?;
    let tally = sweep(&covered, &routed, unit, None);
    let mut spaces: HashMap<u32, Space> = HashMap::new();
    let parse = |key: &str| key.trim_start_matches("AS").parse::<u32>().ok();
    for ((asn, _), weight) in &tally.pairs {
        if let Some(asn) = parse(asn) {
            spaces.entry(asn).or_default().announced += weight;
        }
    }
    for (asn, weight) in &tally.missing {
        if let Some(asn) = parse(asn) {
            let space = spaces.entry(asn).or_default();
            space.announced += weight;
            space.uncovered += weight;
        }
    }
    Ok(spaces)
}

pub fn run(inputs: &Inputs<'_>, output: &Path, top: usize) -> Result<()> {
    let prepared = build::prepare(inputs)?;
    let (asn, _) = merge::asn_db(
        &prepared.selected,
        &prepared.names,
        prepared.policy,
        prepared.epoch,
    );
    let (located, _, _) = merge::location_dbs(
        Vec::new(),
        Vec::new(),
        vec![prepared.listed, prepared.anchored],
        prepared.epoch,
    );
    let asn = reader(asn)?;
    let located = reader(located)?;
    let v4 = spaces(&asn, &located, "0.0.0.0/0", 1.0)?;
    let v6 = spaces(&asn, &located, "2000::/3", 2f64.powi(80))?;

    let mut asns: Vec<u32> = v4
        .keys()
        .chain(v6.keys())
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let get = |map: &HashMap<u32, Space>, asn: u32| map.get(&asn).copied().unwrap_or_default();
    asns.sort_by(|a, b| {
        get(&v4, *b)
            .uncovered
            .total_cmp(&get(&v4, *a).uncovered)
            .then(get(&v6, *b).uncovered.total_cmp(&get(&v6, *a).uncovered))
            .then(a.cmp(b))
    });

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = csv::Writer::from_path(output)?;
    out.write_record([
        "asn",
        "name",
        "country",
        "ipv4_announced",
        "ipv4_without_geofeed",
        "ipv4_share_without_geofeed",
        "ipv6_announced_48",
        "ipv6_without_geofeed_48",
    ])?;
    let (mut total_v4, mut uncovered_v4) = (0.0, 0.0);
    for &asn in &asns {
        let (a, b) = (get(&v4, asn), get(&v6, asn));
        total_v4 += a.announced;
        uncovered_v4 += a.uncovered;
        let name = prepared.names.get(&asn);
        let label = name
            .map(|n| n.organization.as_deref().unwrap_or(&n.handle))
            .unwrap_or_default();
        let country = name.and_then(|n| n.country.as_deref()).unwrap_or_default();
        let share = if a.announced > 0.0 {
            a.uncovered / a.announced
        } else {
            0.0
        };
        out.write_record([
            asn.to_string(),
            label.to_string(),
            country.to_string(),
            format!("{:.0}", a.announced),
            format!("{:.0}", a.uncovered),
            format!("{share:.4}"),
            format!("{:.0}", b.announced),
            format!("{:.0}", b.uncovered),
        ])?;
    }
    out.flush()?;

    eprintln!(
        "coverage: {} ASes, {:.1} % of announced IPv4 space without a geofeed, written to {}",
        asns.len(),
        100.0 * uncovered_v4 / total_v4.max(1.0),
        output.display()
    );
    for &asn in asns.iter().take(top) {
        let a = get(&v4, asn);
        let name = prepared
            .names
            .get(&asn)
            .map(|n| n.organization.as_deref().unwrap_or(&n.handle))
            .unwrap_or_default();
        eprintln!(
            "  AS{asn:<10} {:>12.0} IPv4 addresses without a geofeed ({:>5.1} % of its space)  {name}",
            a.uncovered,
            100.0 * a.uncovered / a.announced.max(1.0)
        );
    }
    Ok(())
}
