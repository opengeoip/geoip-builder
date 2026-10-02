use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::Path;

use anyhow::Result;
use clap::ValueEnum;
use ipnetwork::IpNetwork;
use maxminddb::{Reader, WithinOptions, path};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Kind {
    Country,
    Asn,
}

pub type Interval = (u128, u128, String);
type Pair = (String, String);
type Segment = (f64, u128, u128);

const EXAMPLES: usize = 3;

fn address(value: u128, v4: bool) -> IpAddr {
    if v4 {
        IpAddr::V4(Ipv4Addr::from(value as u32))
    } else {
        IpAddr::V6(Ipv6Addr::from(value))
    }
}

pub fn intervals(reader: &Reader<Vec<u8>>, kind: Kind, scope: IpNetwork) -> Result<Vec<Interval>> {
    let mut out = Vec::new();
    for item in reader.within(scope, WithinOptions::default())? {
        let item = item?;
        let key = match kind {
            Kind::Country => item.decode_path::<String>(&path!["country", "iso_code"])?,
            Kind::Asn => item
                .decode_path::<u32>(&path!["autonomous_system_number"])?
                .map(|asn| format!("AS{asn}")),
        };
        let Some(key) = key else { continue };
        let network = item.network()?;
        let (start, bits) = match network.network() {
            IpAddr::V4(addr) => (u128::from(u32::from(addr)), 32),
            IpAddr::V6(addr) => (u128::from(addr), 128),
        };
        let host_bits = bits - u32::from(network.prefix());
        let end = if host_bits == 128 {
            u128::MAX
        } else {
            start + ((1u128 << host_bits) - 1)
        };
        out.push((start, end, key));
    }
    out.sort_by_key(|i| i.0);
    Ok(out)
}

#[derive(Default)]
pub struct Tally {
    pub agree: f64,
    pub disagree: f64,
    pub only_reference: f64,
    pub only_ours: f64,
    pub pairs: HashMap<Pair, f64>,
    pub examples: HashMap<Pair, Vec<Segment>>,
    pub missing: HashMap<String, f64>,
    pub extra: HashMap<String, f64>,
}

pub fn sweep(ours: &[Interval], reference: &[Interval], unit: f64, only: Option<&str>) -> Tally {
    let mut tally = Tally::default();
    let (mut i, mut j) = (0, 0);
    let mut pos: u128 = 0;
    loop {
        while i < ours.len() && ours[i].1 < pos {
            i += 1;
        }
        while j < reference.len() && reference[j].1 < pos {
            j += 1;
        }
        let (a, b) = (ours.get(i), reference.get(j));
        if a.is_none() && b.is_none() {
            break;
        }
        let covering_a = a.filter(|x| x.0 <= pos);
        let covering_b = b.filter(|x| x.0 <= pos);
        if covering_a.is_none() && covering_b.is_none() {
            pos = [a, b].into_iter().flatten().map(|x| x.0).min().unwrap();
            continue;
        }
        let end = [a, b]
            .into_iter()
            .flatten()
            .map(|x| if x.0 > pos { x.0 - 1 } else { x.1 })
            .min()
            .unwrap();
        let weight = (end - pos) as f64 / unit + 1.0 / unit;
        let selected = only.is_none_or(|key| {
            [covering_a, covering_b]
                .into_iter()
                .flatten()
                .any(|x| x.2 == key)
        });
        match (covering_a, covering_b) {
            _ if !selected => {}
            (Some(a), Some(b)) if a.2 == b.2 => tally.agree += weight,
            (Some(a), Some(b)) => {
                tally.disagree += weight;
                let pair = (b.2.clone(), a.2.clone());
                *tally.pairs.entry(pair.clone()).or_default() += weight;
                let examples = tally.examples.entry(pair).or_default();
                examples.push((weight, pos, end));
                if examples.len() > 2 * EXAMPLES {
                    examples.sort_by(|a, b| b.0.total_cmp(&a.0));
                    examples.truncate(EXAMPLES);
                }
            }
            (None, Some(b)) => {
                tally.only_reference += weight;
                *tally.missing.entry(b.2.clone()).or_default() += weight;
            }
            (Some(a), None) => {
                tally.only_ours += weight;
                *tally.extra.entry(a.2.clone()).or_default() += weight;
            }
            (None, None) => unreachable!(),
        }
        if end == u128::MAX {
            break;
        }
        pos = end + 1;
    }
    tally
}

fn top<K: std::fmt::Debug>(map: &HashMap<K, f64>, n: usize) -> Vec<(&K, f64)> {
    let mut items: Vec<(&K, f64)> = map.iter().map(|(k, v)| (k, *v)).collect();
    items.sort_by(|a, b| b.1.total_cmp(&a.1));
    items.truncate(n);
    items
}

fn report(label: &str, unit: &str, v4: bool, tally: &Tally, limit: usize) {
    let reference = tally.agree + tally.disagree + tally.only_reference;
    let pct = |v: f64| {
        if reference > 0.0 {
            100.0 * v / reference
        } else {
            0.0
        }
    };
    println!("{label}, weighted by {unit}");
    println!("  reference coverage  {reference:>18.0}");
    println!(
        "  agree               {:>18.0}  {:>6.2} %",
        tally.agree,
        pct(tally.agree)
    );
    println!(
        "  disagree            {:>18.0}  {:>6.2} %",
        tally.disagree,
        pct(tally.disagree)
    );
    println!(
        "  only in reference   {:>18.0}  {:>6.2} %",
        tally.only_reference,
        pct(tally.only_reference)
    );
    println!(
        "  only in ours        {:>18.0}  {:>6.2} % of reference size",
        tally.only_ours,
        pct(tally.only_ours)
    );
    println!("  top disagreements (reference -> ours):");
    for ((reference, ours), weight) in top(&tally.pairs, limit) {
        println!(
            "    {reference:>10} -> {ours:<10} {weight:>14.0}  {:>6.2} %",
            pct(weight)
        );
        let mut examples = tally.examples[&(reference.clone(), ours.clone())].clone();
        examples.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (_, start, end) in examples.iter().take(EXAMPLES) {
            println!("        {} - {}", address(*start, v4), address(*end, v4));
        }
    }
    println!("  top missing:");
    for (key, weight) in top(&tally.missing, limit) {
        println!("    {key:>10} {weight:>14.0}  {:>6.2} %", pct(weight));
    }
    println!("  top extra:");
    for (key, weight) in top(&tally.extra, limit) {
        println!("    {key:>10} {weight:>14.0}  {:>6.2} %", pct(weight));
    }
    println!();
}

pub fn run(
    kind: Kind,
    ours: &Path,
    reference: &Path,
    limit: usize,
    only: Option<&str>,
) -> Result<()> {
    let ours = Reader::open_readfile(ours)?;
    let reference = Reader::open_readfile(reference)?;
    for (label, unit_label, scope, unit, v4) in [
        ("IPv4", "addresses", "0.0.0.0/0", 1.0, true),
        (
            "IPv6 (2000::/3)",
            "/48 networks",
            "2000::/3",
            2f64.powi(80),
            false,
        ),
    ] {
        let scope: IpNetwork = scope.parse()?;
        let a = intervals(&ours, kind, scope)?;
        let b = intervals(&reference, kind, scope)?;
        report(label, unit_label, v4, &sweep(&a, &b, unit, only), limit);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sweeps_overlapping_intervals() {
        let ours = vec![(0, 9, "FR".to_string()), (20, 29, "DE".to_string())];
        let reference = vec![(5, 24, "FR".to_string())];
        let tally = sweep(&ours, &reference, 1.0, None);
        assert_eq!(tally.agree, 5.0);
        assert_eq!(tally.only_reference, 10.0);
        assert_eq!(tally.disagree, 5.0);
        assert_eq!(tally.only_ours, 10.0);
        let tally = sweep(&ours, &reference, 1.0, Some("DE"));
        assert_eq!(tally.agree, 0.0);
        assert_eq!(tally.disagree, 5.0);
        assert_eq!(tally.only_ours, 5.0);
    }
}
