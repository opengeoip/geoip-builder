use std::collections::{BTreeMap, HashMap};

use ipnet::IpNet;
use model::{Delegation, GeofeedRef, Location, PrefixMap, Registry};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct GeofeedStats {
    pub entries: usize,
    pub accepted: usize,
    pub no_country: usize,
    pub not_anchored: usize,
    pub overridden: usize,
}

fn bounds(network: IpNet) -> (bool, u128, u128) {
    match network {
        IpNet::V4(n) => (
            false,
            u32::from(n.network()).into(),
            u32::from(n.broadcast()).into(),
        ),
        IpNet::V6(n) => (true, n.network().into(), n.broadcast().into()),
    }
}

pub fn anchored_in_their_registry(
    references: &[(Registry, GeofeedRef)],
    delegations: &[Delegation],
) -> (Vec<GeofeedRef>, usize) {
    let mut spans: BTreeMap<(Registry, bool), Vec<(u128, u128)>> = BTreeMap::new();
    for delegation in delegations {
        let (v6, start, end) = bounds(delegation.network);
        spans
            .entry((delegation.registry, v6))
            .or_default()
            .push((start, end));
    }
    for ranges in spans.values_mut() {
        ranges.sort_unstable();
        let mut merged: Vec<(u128, u128)> = Vec::with_capacity(ranges.len());
        for &(start, end) in ranges.iter() {
            match merged.last_mut() {
                Some(last) if start <= last.1.saturating_add(1) => last.1 = last.1.max(end),
                _ => merged.push((start, end)),
            }
        }
        *ranges = merged;
    }
    let kept: Vec<GeofeedRef> = references
        .iter()
        .filter(|(registry, reference)| {
            let (v6, start, end) = bounds(reference.network);
            spans.get(&(*registry, v6)).is_some_and(|ranges| {
                let index = ranges.partition_point(|(first, _)| *first <= start);
                index > 0 && ranges[index - 1].1 >= end
            })
        })
        .map(|(_, reference)| reference.clone())
        .collect();
    let dropped = references.len() - kept.len();
    (kept, dropped)
}

pub fn authorize_geofeeds(
    references: &[GeofeedRef],
    feeds: &HashMap<String, Vec<Location>>,
) -> (Vec<Location>, GeofeedStats) {
    let mut anchors: PrefixMap<Vec<&str>> = PrefixMap::default();
    for reference in references {
        let urls = anchors.entry_or_default(reference.network);
        if !urls.contains(&reference.url.as_str()) {
            urls.push(&reference.url);
        }
    }

    let mut stats = GeofeedStats::default();
    let mut accepted = Vec::new();
    let mut urls: Vec<&String> = feeds.keys().collect();
    urls.sort();
    for url in urls {
        for location in &feeds[url] {
            stats.entries += 1;
            if location.country.is_none() {
                stats.no_country += 1;
                continue;
            }
            let mut covering = anchors.covering(location.network);
            match covering.next() {
                Some((_, owners)) if owners.contains(&url.as_str()) => {
                    stats.accepted += 1;
                    accepted.push(location.clone());
                }
                Some(_) if covering.any(|(_, owners)| owners.contains(&url.as_str())) => {
                    stats.overridden += 1
                }
                _ => stats.not_anchored += 1,
            }
        }
    }
    (accepted, stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn net(s: &str) -> IpNet {
        s.parse().unwrap()
    }

    fn location(network: &str, country: Option<&str>) -> Location {
        Location {
            network: net(network),
            country: country.map(str::to_string),
            region: None,
            city: None,
            postal: None,
        }
    }

    fn reference(network: &str, url: &str) -> GeofeedRef {
        GeofeedRef {
            network: net(network),
            url: url.into(),
        }
    }

    #[test]
    fn keeps_anchors_inside_a_delegation_of_their_registry() {
        let delegation = |network: &str, registry| Delegation {
            network: net(network),
            country: "US".into(),
            registry,
        };
        let delegations = [
            delegation("2.0.0.0/8", Registry::RipeNcc),
            delegation("3.0.0.0/8", Registry::Arin),
            delegation("153.79.72.0/21", Registry::Arin),
            delegation("153.79.72.0/21", Registry::RipeNcc),
            delegation("5.0.0.0/9", Registry::RipeNcc),
            delegation("5.128.0.0/9", Registry::RipeNcc),
        ];
        let references = [
            (
                Registry::RipeNcc,
                reference("2.152.0.0/16", "https://isp.example/a.csv"),
            ),
            (
                Registry::Lacnic,
                reference("2.152.34.0/23", "https://other.example/b.csv"),
            ),
            (
                Registry::Arin,
                reference("2.153.0.0/24", "https://other.example/c.csv"),
            ),
            (
                Registry::RipeNcc,
                reference("153.79.72.0/24", "https://lir.example/d.csv"),
            ),
            (
                Registry::Arin,
                reference("4.0.0.0/24", "https://other.example/e.csv"),
            ),
            (
                Registry::RipeNcc,
                reference("5.0.0.0/8", "https://isp.example/f.csv"),
            ),
        ];
        let (kept, dropped) = anchored_in_their_registry(&references, &delegations);
        let kept: Vec<String> = kept.iter().map(|r| r.network.to_string()).collect();
        assert_eq!(kept, ["2.152.0.0/16", "153.79.72.0/24", "5.0.0.0/8"]);
        assert_eq!(dropped, 3);
    }

    #[test]
    fn applies_rfc9632_authorization() {
        let references = vec![
            GeofeedRef {
                network: net("11.0.0.0/16"),
                url: "https://a.example/feed.csv".into(),
            },
            GeofeedRef {
                network: net("11.0.128.0/17"),
                url: "https://b.example/feed.csv".into(),
            },
        ];
        let feeds = HashMap::from([
            (
                "https://a.example/feed.csv".to_string(),
                vec![
                    location("11.0.1.0/24", Some("FR")),
                    location("11.0.200.0/24", Some("FR")),
                    location("12.0.0.0/24", Some("FR")),
                    location("11.0.0.0/8", Some("FR")),
                    location("11.0.2.0/24", None),
                ],
            ),
            (
                "https://b.example/feed.csv".to_string(),
                vec![location("11.0.200.0/24", Some("DE"))],
            ),
        ]);
        let (accepted, stats) = authorize_geofeeds(&references, &feeds);
        let accepted: Vec<String> = accepted
            .iter()
            .map(|l| format!("{} {}", l.network, l.country.as_deref().unwrap()))
            .collect();
        assert_eq!(accepted, ["11.0.1.0/24 FR", "11.0.200.0/24 DE"]);
        assert_eq!(
            stats,
            GeofeedStats {
                entries: 6,
                accepted: 2,
                no_country: 1,
                not_anchored: 2,
                overridden: 1,
            }
        );
    }
}
