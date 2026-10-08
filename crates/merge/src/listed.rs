use std::collections::{BTreeMap, HashMap};

use ipnet::IpNet;
use model::{Asn, Location, PrefixMap};

pub struct ListedGeofeed {
    pub url: String,
    pub asns: Vec<Asn>,
    pub locations: Vec<Location>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ListedFeed {
    pub url: String,
    pub entries: usize,
    pub accepted: usize,
    pub unrouted: usize,
    pub undeclared: Vec<(Asn, usize)>,
}

fn inner_origins<'a>(
    routes: &'a BTreeMap<IpNet, Asn>,
    network: IpNet,
) -> impl Iterator<Item = Asn> + 'a {
    routes
        .range(network..)
        .take_while(move |(route, _)| network.contains(*route))
        .map(|(_, asn)| *asn)
}

pub fn authorize_listed_geofeeds(
    feeds: &[ListedGeofeed],
    origins: &PrefixMap<Asn>,
) -> (Vec<Location>, Vec<ListedFeed>) {
    let routes: BTreeMap<IpNet, Asn> = origins
        .iter()
        .map(|(network, asn)| (network, *asn))
        .collect();
    let mut accepted = Vec::new();
    let mut reports = Vec::new();
    for feed in feeds {
        let mut report = ListedFeed {
            url: feed.url.clone(),
            entries: feed.locations.len(),
            ..Default::default()
        };
        let mut undeclared: HashMap<Asn, usize> = HashMap::new();
        let mut aggregates: Vec<&Location> = Vec::new();
        for location in feed.locations.iter().filter(|l| l.country.is_some()) {
            match origins.longest_match(location.network) {
                Some((_, asn)) if feed.asns.contains(asn) => {
                    report.accepted += 1;
                    accepted.push(location.clone());
                }
                Some((_, asn)) => *undeclared.entry(*asn).or_default() += 1,
                None if inner_origins(&routes, location.network).next().is_some() => {
                    aggregates.push(location)
                }
                None => report.unrouted += 1,
            }
        }
        for location in aggregates {
            let mut owned = true;
            for asn in inner_origins(&routes, location.network) {
                if !feed.asns.contains(&asn) {
                    *undeclared.entry(asn).or_default() += 1;
                    owned = false;
                }
            }
            if owned {
                report.accepted += 1;
                accepted.push(location.clone());
            }
        }
        report.undeclared = undeclared.into_iter().collect();
        report
            .undeclared
            .sort_unstable_by_key(|(asn, count)| (std::cmp::Reverse(*count), *asn));
        reports.push(report);
    }
    (accepted, reports)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn location(network: &str) -> Location {
        Location {
            network: network.parse().unwrap(),
            country: Some("US".into()),
            region: None,
            city: None,
            postal: None,
        }
    }

    fn origins() -> PrefixMap<Asn> {
        [
            ("11.0.0.0/16", 16509),
            ("11.1.0.0/16", 16509),
            ("11.2.0.0/16", 8987),
            ("11.3.0.0/16", 64999),
            ("13.0.0.0/16", 16509),
            ("13.1.0.0/16", 8987),
            ("13.2.0.0/16", 16509),
            ("13.3.0.0/16", 64999),
        ]
        .into_iter()
        .map(|(n, a)| (n.parse::<IpNet>().unwrap(), a))
        .collect()
    }

    fn feed(asns: &[Asn], networks: &[&str]) -> ListedGeofeed {
        ListedGeofeed {
            url: "https://feed.example/geo.csv".into(),
            asns: asns.to_vec(),
            locations: networks.iter().map(|n| location(n)).collect(),
        }
    }

    #[test]
    fn keeps_only_entries_announced_by_the_declared_asns() {
        let feeds = [feed(
            &[8987, 16509],
            &[
                "11.0.1.0/24",
                "11.1.1.0/24",
                "11.2.1.0/24",
                "11.3.1.0/24",
                "12.0.0.0/24",
                "13.0.0.0/15",
                "13.2.0.0/15",
            ],
        )];
        let (accepted, reports) = authorize_listed_geofeeds(&feeds, &origins());
        let accepted: Vec<String> = accepted.iter().map(|l| l.network.to_string()).collect();
        assert_eq!(
            accepted,
            ["11.0.1.0/24", "11.1.1.0/24", "11.2.1.0/24", "13.0.0.0/15"]
        );
        assert_eq!(
            reports,
            [ListedFeed {
                url: "https://feed.example/geo.csv".into(),
                entries: 7,
                accepted: 4,
                unrouted: 1,
                undeclared: vec![(64999, 2)],
            }]
        );
    }

    #[test]
    fn ignores_what_the_feed_says_about_other_networks() {
        let feeds = [feed(
            &[64999],
            &["11.0.1.0/24", "11.1.1.0/24", "11.2.1.0/24"],
        )];
        let (accepted, reports) = authorize_listed_geofeeds(&feeds, &origins());
        assert!(accepted.is_empty());
        assert_eq!(reports[0].undeclared, [(16509, 2), (8987, 1)]);
    }
}
