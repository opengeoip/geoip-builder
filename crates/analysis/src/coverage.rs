use std::collections::{BTreeSet, HashMap};

use anyhow::Result;
use ipnetwork::IpNetwork;
use maxminddb::Reader;

use crate::compare::{Kind, families, intervals, sweep};

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Space {
    pub announced: f64,
    pub uncovered: f64,
}

impl Space {
    pub fn share_uncovered(&self) -> f64 {
        if self.announced > 0.0 {
            self.uncovered / self.announced
        } else {
            0.0
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AsCoverage {
    pub asn: u32,
    pub ipv4: Space,
    pub ipv6: Space,
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

pub fn coverage(asn: &Reader<Vec<u8>>, located: &Reader<Vec<u8>>) -> Result<Vec<AsCoverage>> {
    let [ipv4, ipv6] = families();
    let v4 = spaces(asn, located, ipv4.scope, ipv4.weight)?;
    let v6 = spaces(asn, located, ipv6.scope, ipv6.weight)?;
    let asns: BTreeSet<u32> = v4.keys().chain(v6.keys()).copied().collect();
    let get = |map: &HashMap<u32, Space>, asn: u32| map.get(&asn).copied().unwrap_or_default();
    let mut rows: Vec<AsCoverage> = asns
        .into_iter()
        .map(|asn| AsCoverage {
            asn,
            ipv4: get(&v4, asn),
            ipv6: get(&v6, asn),
        })
        .collect();
    rows.sort_by(|a, b| {
        b.ipv4
            .uncovered
            .total_cmp(&a.ipv4.uncovered)
            .then(b.ipv6.uncovered.total_cmp(&a.ipv6.uncovered))
            .then(a.asn.cmp(&b.asn))
    });
    Ok(rows)
}

pub fn ipv4_share_uncovered(rows: &[AsCoverage]) -> f64 {
    let (total, uncovered) = rows.iter().fold((0.0, 0.0), |(total, uncovered), row| {
        (total + row.ipv4.announced, uncovered + row.ipv4.uncovered)
    });
    100.0 * uncovered / f64::max(total, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{asn, country, database};

    #[test]
    fn measures_space_without_geofeed_per_as() {
        let routes = database(&[
            ("10.0.0.0/8", asn(64500)),
            ("11.0.0.0/8", asn(64501)),
            ("2a00::/16", asn(64500)),
        ]);
        let located = database(&[("10.0.0.0/9", country("FR")), ("2a00::/17", country("FR"))]);
        let rows = coverage(&routes, &located).unwrap();
        assert_eq!(
            rows.iter().map(|r| r.asn).collect::<Vec<_>>(),
            [64501, 64500]
        );
        assert_eq!(
            rows[0].ipv4,
            Space {
                announced: 16_777_216.0,
                uncovered: 16_777_216.0
            }
        );
        assert_eq!(
            rows[1].ipv4,
            Space {
                announced: 16_777_216.0,
                uncovered: 8_388_608.0
            }
        );
        assert_eq!(rows[1].ipv4.share_uncovered(), 0.5);
        assert_eq!(rows[1].ipv6.announced, 2f64.powi(32));
        assert_eq!(rows[1].ipv6.uncovered, 2f64.powi(31));
        assert_eq!(ipv4_share_uncovered(&rows), 75.0);
    }
}
