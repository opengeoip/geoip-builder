use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use anyhow::Result;
use ipnetwork::IpNetwork;
use maxminddb::{Reader, WithinOptions, path};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Country,
    Asn,
}

pub type Interval = (u128, u128, String);
pub type Pair = (String, String);
pub type Segment = (f64, u128, u128);

pub const EXAMPLES: usize = 3;

pub struct Family {
    pub label: &'static str,
    pub unit: &'static str,
    pub scope: &'static str,
    pub weight: f64,
    pub v4: bool,
}

pub fn families() -> [Family; 2] {
    [
        Family {
            label: "IPv4",
            unit: "addresses",
            scope: "0.0.0.0/0",
            weight: 1.0,
            v4: true,
        },
        Family {
            label: "IPv6 (2000::/3)",
            unit: "/48 networks",
            scope: "2000::/3",
            weight: 2f64.powi(80),
            v4: false,
        },
    ]
}

pub fn address(value: u128, v4: bool) -> IpAddr {
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

impl Tally {
    pub fn reference(&self) -> f64 {
        self.agree + self.disagree + self.only_reference
    }

    pub fn share(&self, value: f64) -> f64 {
        let reference = self.reference();
        if reference > 0.0 {
            100.0 * value / reference
        } else {
            0.0
        }
    }

    pub fn examples(&self, pair: &Pair) -> Vec<Segment> {
        let mut examples = self.examples[pair].clone();
        examples.sort_by(|a, b| b.0.total_cmp(&a.0));
        examples.truncate(EXAMPLES);
        examples
    }
}

pub fn top<K>(map: &HashMap<K, f64>, n: usize) -> Vec<(&K, f64)> {
    let mut items: Vec<(&K, f64)> = map.iter().map(|(k, v)| (k, *v)).collect();
    items.sort_by(|a, b| b.1.total_cmp(&a.1));
    items.truncate(n);
    items
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

pub fn compare(
    ours: &Reader<Vec<u8>>,
    reference: &Reader<Vec<u8>>,
    kind: Kind,
    family: &Family,
    only: Option<&str>,
) -> Result<Tally> {
    let scope: IpNetwork = family.scope.parse()?;
    let a = intervals(ours, kind, scope)?;
    let b = intervals(reference, kind, scope)?;
    Ok(sweep(&a, &b, family.weight, only))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{country, database};

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

    #[test]
    fn compares_two_databases() {
        let ours = database(&[("10.0.0.0/8", country("FR")), ("11.0.0.0/8", country("DE"))]);
        let reference = database(&[("10.0.0.0/8", country("FR")), ("11.0.0.0/9", country("NL"))]);
        let family = &families()[0];
        let tally = compare(&ours, &reference, Kind::Country, family, None).unwrap();
        assert_eq!(tally.agree, 16_777_216.0);
        assert_eq!(tally.disagree, 8_388_608.0);
        assert_eq!(tally.only_ours, 8_388_608.0);
        let pair = ("NL".to_string(), "DE".to_string());
        assert_eq!(tally.pairs[&pair], 8_388_608.0);
        let examples = tally.examples(&pair);
        assert_eq!(address(examples[0].1, true).to_string(), "11.0.0.0");
        assert!((tally.share(tally.agree) - 66.666).abs() < 0.01);
        assert_eq!(top(&tally.pairs, 5).len(), 1);
    }
}
