use std::collections::HashMap;

use anyhow::Result;
use maxminddb::Reader;
use model::continent::{Continent, continent};
use src_atlas::Probe;

use crate::lookup;

pub type Pair = (String, String);

pub struct Group {
    pub label: String,
    pub ipv4: bool,
    pub anchors: Option<bool>,
}

impl Group {
    pub fn is_detailed(&self) -> bool {
        self.anchors.is_none()
    }

    pub fn select<'a>(&self, probes: &'a [Probe]) -> Vec<&'a Probe> {
        probes
            .iter()
            .filter(|p| {
                p.address.is_ipv4() == self.ipv4 && self.anchors.is_none_or(|a| p.is_anchor == a)
            })
            .collect()
    }
}

pub fn groups() -> Vec<Group> {
    [("IPv4", true), ("IPv6", false)]
        .into_iter()
        .flat_map(|(family, ipv4)| {
            [
                ("", None),
                (" anchors", Some(true)),
                (" probes", Some(false)),
            ]
            .into_iter()
            .map(move |(suffix, anchors)| Group {
                label: format!("{family}{suffix}"),
                ipv4,
                anchors,
            })
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Network {
    Access,
    Transit,
    Content,
    Other,
    Unknown,
    Unrouted,
}

impl Network {
    pub const ALL: [Network; 6] = [
        Network::Access,
        Network::Transit,
        Network::Content,
        Network::Other,
        Network::Unknown,
        Network::Unrouted,
    ];

    pub fn from_peeringdb(info_type: Option<&str>) -> Network {
        match info_type {
            Some("Cable/DSL/ISP") => Network::Access,
            Some("NSP") => Network::Transit,
            Some("Content") => Network::Content,
            None | Some("") => Network::Unknown,
            Some(_) => Network::Other,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Network::Access => "access",
            Network::Transit => "transit",
            Network::Content => "content",
            Network::Other => "other",
            Network::Unknown => "unknown",
            Network::Unrouted => "unrouted",
        }
    }
}

pub struct Bucket<'a> {
    pub label: &'static str,
    pub probes: Vec<&'a Probe>,
}

fn buckets<'a, K: Copy + Eq>(
    probes: &[&'a Probe],
    keys: impl IntoIterator<Item = (K, &'static str)>,
    key: impl Fn(&Probe) -> K,
) -> Vec<Bucket<'a>> {
    keys.into_iter()
        .map(|(k, label)| Bucket {
            label,
            probes: probes.iter().copied().filter(|p| key(p) == k).collect(),
        })
        .filter(|b| !b.probes.is_empty())
        .collect()
}

pub fn by_network<'a>(probes: &[&'a Probe], networks: &HashMap<u32, Network>) -> Vec<Bucket<'a>> {
    buckets(probes, Network::ALL.map(|n| (n, n.as_str())), |p| {
        match p.asn {
            None => Network::Unrouted,
            Some(asn) => networks.get(&asn).copied().unwrap_or(Network::Unknown),
        }
    })
}

pub fn by_continent<'a>(probes: &[&'a Probe]) -> Vec<Bucket<'a>> {
    let keys = Continent::ALL
        .map(|c| (Some(c), c.name()))
        .into_iter()
        .chain([(None, "unknown")]);
    buckets(probes, keys, |p| continent(&p.country))
}

pub fn answers(reader: &Reader<Vec<u8>>, probes: &[&Probe]) -> Result<Vec<Option<String>>> {
    probes
        .iter()
        .map(|p| lookup::country(reader, p.address))
        .collect()
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Score {
    pub correct: usize,
    pub wrong: usize,
    pub missing: usize,
    pub errors: Vec<(Pair, Vec<u32>)>,
}

pub fn percent(part: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        100.0 * part as f64 / total as f64
    }
}

pub fn score(probes: &[&Probe], answers: &[Option<String>]) -> Score {
    let mut score = Score::default();
    let mut errors: HashMap<Pair, Vec<u32>> = HashMap::new();
    for (probe, answer) in probes.iter().zip(answers) {
        match answer {
            Some(answer) if *answer == probe.country => score.correct += 1,
            Some(answer) => {
                score.wrong += 1;
                errors
                    .entry((probe.country.clone(), answer.clone()))
                    .or_default()
                    .push(probe.id);
            }
            None => score.missing += 1,
        }
    }
    score.errors = errors.into_iter().collect();
    score
        .errors
        .sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
    score
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct HeadToHead {
    pub both: usize,
    pub only_first: usize,
    pub only_second: usize,
    pub neither: usize,
    pub only_second_right: Vec<usize>,
}

pub fn head_to_head(
    probes: &[&Probe],
    first: &[Option<String>],
    second: &[Option<String>],
) -> HeadToHead {
    let mut result = HeadToHead::default();
    for (index, probe) in probes.iter().enumerate() {
        let right =
            |answers: &[Option<String>]| answers[index].as_deref() == Some(probe.country.as_str());
        match (right(first), right(second)) {
            (true, true) => result.both += 1,
            (true, false) => result.only_first += 1,
            (false, true) => {
                result.only_second += 1;
                result.only_second_right.push(index);
            }
            (false, false) => result.neither += 1,
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe(id: u32, address: &str, country: &str, is_anchor: bool) -> Probe {
        Probe {
            id,
            address: address.parse().unwrap(),
            country: country.into(),
            is_anchor,
            asn: Some(id),
            auto_located: false,
        }
    }

    fn answer(code: Option<&str>) -> Option<String> {
        code.map(str::to_string)
    }

    #[test]
    fn scores_and_compares_two_databases() {
        let probes = [
            probe(1, "192.0.2.1", "FR", false),
            probe(2, "192.0.2.2", "FR", false),
            probe(3, "192.0.2.3", "DE", true),
            probe(4, "192.0.2.4", "DE", false),
            probe(5, "192.0.2.5", "NL", false),
        ];
        let selected: Vec<&Probe> = probes.iter().collect();
        let first = [
            answer(Some("FR")),
            answer(Some("US")),
            answer(Some("US")),
            answer(Some("DE")),
            answer(None),
        ];
        let second = [
            answer(Some("FR")),
            answer(Some("FR")),
            answer(Some("NL")),
            answer(Some("NL")),
            answer(Some("NL")),
        ];
        let score = score(&selected, &first);
        assert_eq!(score.correct, 2);
        assert_eq!(score.wrong, 2);
        assert_eq!(score.missing, 1);
        assert_eq!(
            score.errors,
            [
                (("DE".to_string(), "US".to_string()), vec![3]),
                (("FR".to_string(), "US".to_string()), vec![2]),
            ]
        );
        let duel = head_to_head(&selected, &first, &second);
        assert_eq!(
            duel,
            HeadToHead {
                both: 1,
                only_first: 1,
                only_second: 2,
                neither: 1,
                only_second_right: vec![1, 4],
            }
        );
        assert_eq!(percent(1, 4), 25.0);
        assert_eq!(percent(1, 0), 0.0);
    }

    #[test]
    fn splits_probes_into_groups() {
        let probes = [
            probe(1, "192.0.2.1", "FR", true),
            probe(2, "192.0.2.2", "FR", false),
            probe(3, "2001:db8::1", "FR", true),
        ];
        let groups = groups();
        let labels: Vec<&str> = groups.iter().map(|g| g.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "IPv4",
                "IPv4 anchors",
                "IPv4 probes",
                "IPv6",
                "IPv6 anchors",
                "IPv6 probes"
            ]
        );
        let counts: Vec<usize> = groups.iter().map(|g| g.select(&probes).len()).collect();
        assert_eq!(counts, [2, 1, 1, 1, 1, 0]);
        assert!(groups[0].is_detailed() && !groups[1].is_detailed());
    }

    #[test]
    fn breaks_probes_down_by_network_and_continent() {
        let probes = [
            probe(1, "192.0.2.1", "FR", false),
            probe(2, "192.0.2.2", "BR", false),
            probe(3, "192.0.2.3", "FR", false),
            probe(4, "192.0.2.4", "XK", false),
            Probe {
                asn: None,
                ..probe(5, "192.0.2.5", "FR", false)
            },
        ];
        let selected: Vec<&Probe> = probes.iter().collect();
        let networks = HashMap::from([
            (1, Network::from_peeringdb(Some("Cable/DSL/ISP"))),
            (2, Network::from_peeringdb(Some("Content"))),
            (3, Network::from_peeringdb(Some("Educational/Research"))),
        ]);
        let summary = |buckets: Vec<Bucket<'_>>| -> Vec<(&str, Vec<u32>)> {
            buckets
                .into_iter()
                .map(|b| (b.label, b.probes.iter().map(|p| p.id).collect()))
                .collect()
        };
        assert_eq!(
            summary(by_network(&selected, &networks)),
            [
                ("access", vec![1]),
                ("content", vec![2]),
                ("other", vec![3]),
                ("unknown", vec![4]),
                ("unrouted", vec![5])
            ]
        );
        assert_eq!(
            summary(by_continent(&selected)),
            [
                ("Europe", vec![1, 3, 5]),
                ("South America", vec![2]),
                ("unknown", vec![4])
            ]
        );
    }
}
