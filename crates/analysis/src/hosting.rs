use std::collections::HashMap;

use anyhow::Result;
use maxminddb::Reader;
use src_atlas::Probe;

use crate::lookup;

pub const HOSTED_TAGS: [&str; 9] = [
    "datacentre",
    "datacenter",
    "data-center",
    "vps",
    "cloud",
    "hosting",
    "colo",
    "colocation",
    "server",
];

pub const ACCESS_TAGS: [&str; 17] = [
    "home",
    "residential",
    "fibre",
    "ftth",
    "gpon",
    "fttc",
    "cable",
    "docsis",
    "dsl",
    "adsl",
    "vdsl",
    "vdsl2",
    "pppoe",
    "lte",
    "4g",
    "5g",
    "mobile",
];

pub fn hosted(probe: &Probe) -> Option<bool> {
    let has = |tags: &[&str]| probe.tags.iter().any(|t| tags.contains(&t.as_str()));
    match (has(&HOSTED_TAGS), has(&ACCESS_TAGS)) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => None,
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct HostingScore {
    pub true_positive: usize,
    pub false_positive: usize,
    pub false_negative: usize,
    pub true_negative: usize,
    pub false_positive_ases: Vec<(u32, usize)>,
    pub false_negative_ases: Vec<(u32, usize)>,
}

impl HostingScore {
    pub fn precision(&self) -> f64 {
        ratio(self.true_positive, self.true_positive + self.false_positive)
    }

    pub fn recall(&self) -> f64 {
        ratio(self.true_positive, self.true_positive + self.false_negative)
    }
}

fn ratio(part: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        100.0 * part as f64 / total as f64
    }
}

fn ranked(counts: HashMap<u32, usize>) -> Vec<(u32, usize)> {
    let mut ranked: Vec<(u32, usize)> = counts.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    ranked
}

pub fn score(reader: &Reader<Vec<u8>>, probes: &[&Probe]) -> Result<HostingScore> {
    let mut score = HostingScore::default();
    let mut false_positive: HashMap<u32, usize> = HashMap::new();
    let mut false_negative: HashMap<u32, usize> = HashMap::new();
    for probe in probes {
        let Some(truth) = hosted(probe) else {
            continue;
        };
        let flagged = lookup::hosting_provider(reader, probe.address)?;
        let asn = probe.asn.unwrap_or(0);
        match (truth, flagged) {
            (true, true) => score.true_positive += 1,
            (false, true) => {
                score.false_positive += 1;
                *false_positive.entry(asn).or_default() += 1;
            }
            (true, false) => {
                score.false_negative += 1;
                *false_negative.entry(asn).or_default() += 1;
            }
            (false, false) => score.true_negative += 1,
        }
    }
    score.false_positive_ases = ranked(false_positive);
    score.false_negative_ases = ranked(false_negative);
    Ok(score)
}

#[cfg(test)]
mod tests {
    use mmdb_writer::Value;

    use super::*;
    use crate::testing::database;

    fn probe(id: u32, address: &str, tags: &[&str]) -> Probe {
        Probe {
            id,
            address: address.parse().unwrap(),
            country: "FR".into(),
            is_anchor: false,
            asn: Some(64500 + id),
            auto_located: false,
            tags: tags.iter().map(|t| t.to_string()).collect(),
        }
    }

    #[test]
    fn labels_probes_from_their_tags() {
        assert_eq!(
            hosted(&probe(1, "192.0.2.1", &["datacentre", "nat"])),
            Some(true)
        );
        assert_eq!(
            hosted(&probe(1, "192.0.2.1", &["home", "fibre"])),
            Some(false)
        );
        assert_eq!(hosted(&probe(1, "192.0.2.1", &["vps", "home"])), None);
        assert_eq!(hosted(&probe(1, "192.0.2.1", &["office"])), None);
    }

    #[test]
    fn scores_the_hosting_flag() {
        let reader = database(&[(
            "192.0.2.0/25",
            Value::map([("is_hosting_provider", Value::Bool(true))]),
        )]);
        let probes = [
            probe(1, "192.0.2.1", &["datacentre"]),
            probe(2, "192.0.2.2", &["home"]),
            probe(3, "192.0.2.200", &["vps"]),
            probe(4, "192.0.2.201", &["cable"]),
            probe(5, "192.0.2.202", &["office"]),
        ];
        let selected: Vec<&Probe> = probes.iter().collect();
        let score = score(&reader, &selected).unwrap();
        assert_eq!(
            score,
            HostingScore {
                true_positive: 1,
                false_positive: 1,
                false_negative: 1,
                true_negative: 1,
                false_positive_ases: vec![(64502, 1)],
                false_negative_ases: vec![(64503, 1)],
            }
        );
        assert_eq!(score.precision(), 50.0);
        assert_eq!(score.recall(), 50.0);
    }
}
