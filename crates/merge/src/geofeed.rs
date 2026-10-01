use std::collections::HashMap;

use model::{Asn, GeofeedRef, Location, PrefixMap};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct GeofeedStats {
    pub entries: usize,
    pub accepted: usize,
    pub no_country: usize,
    pub not_anchored: usize,
    pub overridden: usize,
    pub seed_accepted: usize,
    pub seed_rejected: usize,
}

pub fn authorize_geofeeds(
    references: &[GeofeedRef],
    feeds: &HashMap<String, Vec<Location>>,
    seeds: &[(&[Asn], Vec<Location>)],
    origins: &PrefixMap<Asn>,
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

    for (asns, locations) in seeds {
        for location in locations {
            stats.entries += 1;
            if location.country.is_none() {
                stats.no_country += 1;
                continue;
            }
            match origins.longest_match(location.network) {
                Some((_, asn)) if asns.contains(asn) => {
                    stats.seed_accepted += 1;
                    accepted.push(location.clone());
                }
                _ => stats.seed_rejected += 1,
            }
        }
    }

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
    use ipnet::IpNet;

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
        let origins: PrefixMap<Asn> = [(net("13.0.0.0/16"), 64500), (net("14.0.0.0/16"), 64999)]
            .into_iter()
            .collect();
        let seeds: Vec<(&[Asn], Vec<Location>)> = vec![(
            &[64500],
            vec![
                location("13.0.1.0/24", Some("JP")),
                location("14.0.1.0/24", Some("JP")),
            ],
        )];
        let (accepted, stats) = authorize_geofeeds(&references, &feeds, &seeds, &origins);
        let accepted: Vec<String> = accepted
            .iter()
            .map(|l| format!("{} {}", l.network, l.country.as_deref().unwrap()))
            .collect();
        assert_eq!(
            accepted,
            ["13.0.1.0/24 JP", "11.0.1.0/24 FR", "11.0.200.0/24 DE"]
        );
        assert_eq!(
            stats,
            GeofeedStats {
                entries: 8,
                accepted: 2,
                no_country: 1,
                not_anchored: 2,
                overridden: 1,
                seed_accepted: 1,
                seed_rejected: 1,
            }
        );
    }
}
