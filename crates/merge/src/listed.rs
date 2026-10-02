use std::collections::{BTreeSet, HashMap};

use model::{AsName, Asn, Location, PrefixMap};

const GENERIC_WORDS: &[&str] = &[
    "access",
    "asia",
    "broadband",
    "cloud",
    "communications",
    "company",
    "corp",
    "corporation",
    "data",
    "datacenter",
    "europe",
    "global",
    "gmbh",
    "group",
    "hosting",
    "inc",
    "infrastructure",
    "international",
    "internet",
    "limited",
    "llc",
    "ltd",
    "media",
    "net",
    "network",
    "networks",
    "online",
    "pacific",
    "private",
    "services",
    "solutions",
    "systems",
    "technologies",
    "technology",
    "telecom",
    "telecommunications",
    "the",
];

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ListedFeed {
    pub url: String,
    pub entries: usize,
    pub accepted: usize,
    pub unrouted: usize,
    pub publishers: Vec<Asn>,
}

fn words(name: &AsName) -> BTreeSet<String> {
    let text = format!(
        "{} {}",
        name.handle,
        name.organization.as_deref().unwrap_or_default()
    );
    text.split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|w| {
            w.len() >= 4
                && !w.bytes().all(|b| b.is_ascii_digit())
                && !GENERIC_WORDS.contains(&w.as_str())
        })
        .collect()
}

pub fn publishers(counts: &HashMap<Asn, usize>, names: &HashMap<Asn, AsName>) -> Vec<Asn> {
    let Some((&dominant, _)) = counts
        .iter()
        .max_by_key(|(asn, count)| (**count, std::cmp::Reverse(**asn)))
    else {
        return Vec::new();
    };
    let reference = names.get(&dominant).map(words).unwrap_or_default();
    let mut publishers: Vec<Asn> = counts
        .keys()
        .copied()
        .filter(|asn| {
            *asn == dominant
                || names
                    .get(asn)
                    .is_some_and(|name| !words(name).is_disjoint(&reference))
        })
        .collect();
    publishers.sort_unstable();
    publishers
}

pub fn authorize_listed_geofeeds(
    feeds: &[(String, Vec<Location>)],
    origins: &PrefixMap<Asn>,
    names: &HashMap<Asn, AsName>,
) -> (Vec<Location>, Vec<ListedFeed>) {
    let mut accepted = Vec::new();
    let mut reports = Vec::new();
    for (url, locations) in feeds {
        let mut report = ListedFeed {
            url: url.clone(),
            ..Default::default()
        };
        let routed: Vec<(&Location, Asn)> = locations
            .iter()
            .filter(|location| location.country.is_some())
            .filter_map(|location| match origins.longest_match(location.network) {
                Some((_, asn)) => Some((location, *asn)),
                None => {
                    report.unrouted += 1;
                    None
                }
            })
            .collect();
        report.entries = locations.len();
        let mut counts: HashMap<Asn, usize> = HashMap::new();
        for (_, asn) in &routed {
            *counts.entry(*asn).or_default() += 1;
        }
        report.publishers = publishers(&counts, names);
        for (location, asn) in routed {
            if report.publishers.contains(&asn) {
                report.accepted += 1;
                accepted.push(location.clone());
            }
        }
        reports.push(report);
    }
    (accepted, reports)
}

#[cfg(test)]
mod tests {
    use ipnet::IpNet;

    use super::*;

    fn name(handle: &str, organization: &str) -> AsName {
        AsName {
            handle: handle.into(),
            organization: Some(organization.into()),
            country: None,
        }
    }

    fn location(network: &str) -> Location {
        Location {
            network: network.parse().unwrap(),
            country: Some("US".into()),
            region: None,
            city: None,
            postal: None,
        }
    }

    #[test]
    fn derives_the_publisher_from_the_dominant_origin_and_its_organization() {
        let names = HashMap::from([
            (13335, name("CLOUDFLARENET", "Cloudflare, Inc.")),
            (14789, name("CLOUDFLARENET-SFO", "Cloudflare, Inc.")),
            (2900, name("ARIZONA-TRI", "Arizona Tri University Network")),
            (64500, name("OTHER", "Cloud Network Services Inc")),
        ]);
        let counts = HashMap::from([(13335, 900), (14789, 5), (2900, 7), (64500, 3)]);
        assert_eq!(publishers(&counts, &names), [13335, 14789]);
    }

    #[test]
    fn keeps_only_entries_announced_by_the_publisher() {
        let names = HashMap::from([
            (16509, name("AMAZON-02", "Amazon.com, Inc.")),
            (8987, name("AMAZON-EXPANSION", "Amazon")),
            (64999, name("CUSTOMER", "Someone Else")),
        ]);
        let origins: PrefixMap<Asn> = [
            ("11.0.0.0/16", 16509),
            ("11.1.0.0/16", 16509),
            ("11.2.0.0/16", 8987),
            ("11.3.0.0/16", 64999),
        ]
        .into_iter()
        .map(|(n, a)| (n.parse::<IpNet>().unwrap(), a))
        .collect();
        let feeds = vec![(
            "https://feed.example/geo.csv".to_string(),
            vec![
                location("11.0.1.0/24"),
                location("11.1.1.0/24"),
                location("11.2.1.0/24"),
                location("11.3.1.0/24"),
                location("12.0.0.0/24"),
            ],
        )];
        let (accepted, reports) = authorize_listed_geofeeds(&feeds, &origins, &names);
        let accepted: Vec<String> = accepted.iter().map(|l| l.network.to_string()).collect();
        assert_eq!(accepted, ["11.0.1.0/24", "11.1.1.0/24", "11.2.1.0/24"]);
        assert_eq!(
            reports,
            [ListedFeed {
                url: "https://feed.example/geo.csv".into(),
                entries: 5,
                accepted: 3,
                unrouted: 1,
                publishers: vec![8987, 16509],
            }]
        );
    }
}
