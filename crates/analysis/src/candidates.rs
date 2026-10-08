use std::collections::HashMap;
use std::io::BufRead;

use anyhow::Result;
use maxminddb::Reader;
use src_atlas::Probe;

use crate::lookup;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Candidate {
    pub probes: usize,
    pub wrong: usize,
    pub errors: HashMap<(String, String), usize>,
}

impl Candidate {
    pub fn top_error(&self) -> String {
        self.errors
            .iter()
            .max_by_key(|(pair, count)| (**count, std::cmp::Reverse((*pair).clone())))
            .map(|((truth, ours), count)| format!("{truth}->{ours} x{count}"))
            .unwrap_or_default()
    }
}

pub struct Observation {
    pub asn: u32,
    pub truth: String,
    pub answer: Option<String>,
}

pub fn observe(
    probes: &[Probe],
    country: &Reader<Vec<u8>>,
    asn: &Reader<Vec<u8>>,
) -> Result<Vec<Observation>> {
    let mut observations = Vec::new();
    for probe in probes {
        let Some(origin) = lookup::asn(asn, probe.address)? else {
            continue;
        };
        observations.push(Observation {
            asn: origin,
            truth: probe.country.clone(),
            answer: lookup::country(country, probe.address)?,
        });
    }
    Ok(observations)
}

pub fn rank(observations: impl IntoIterator<Item = Observation>) -> Vec<(u32, Candidate)> {
    let mut candidates: HashMap<u32, Candidate> = HashMap::new();
    for observation in observations {
        let candidate = candidates.entry(observation.asn).or_default();
        candidate.probes += 1;
        if observation.answer.as_deref() != Some(observation.truth.as_str()) {
            candidate.wrong += 1;
            *candidate
                .errors
                .entry((
                    observation.truth,
                    observation.answer.unwrap_or_else(|| "-".into()),
                ))
                .or_default() += 1;
        }
    }
    let mut ranked: Vec<(u32, Candidate)> = candidates
        .into_iter()
        .filter(|(_, c)| c.wrong > 0)
        .collect();
    ranked.sort_by(|a, b| b.1.wrong.cmp(&a.1.wrong).then(a.0.cmp(&b.0)));
    ranked
}

pub fn website_host(website: &str) -> Option<String> {
    let rest = website.split_once("://").map_or(website, |(_, rest)| rest);
    let host = rest
        .split(['/', ':', '?', '#'])
        .next()?
        .trim()
        .to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host).to_string();
    (host.contains('.') && !host.is_empty()).then_some(host)
}

pub fn guesses(host: &str) -> Vec<String> {
    let mut urls: Vec<String> = [
        "geofeed.csv",
        "geofeed",
        "geofeed.txt",
        ".well-known/geofeed",
        ".well-known/geofeed.csv",
    ]
    .iter()
    .flat_map(|path| {
        [
            format!("https://{host}/{path}"),
            format!("https://www.{host}/{path}"),
        ]
    })
    .collect();
    urls.push(format!("https://geofeed.{host}/"));
    urls.push(format!("https://geofeed.{host}/geofeed.csv"));
    urls
}

pub fn normalized(url: &str) -> String {
    let lower = url.to_ascii_lowercase();
    let rest = lower
        .split_once("://")
        .map_or(lower.as_str(), |(_, rest)| rest);
    rest.strip_prefix("www.")
        .unwrap_or(rest)
        .trim_end_matches('/')
        .to_string()
}

pub fn looks_like_geofeed<R: BufRead>(reader: R) -> bool {
    match src_geofeed::parse(reader) {
        Ok((locations, stats)) => locations.len() >= 10 && stats.invalid * 4 <= stats.entries,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{asn, country, database};

    fn seen(asn: u32, truth: &str, answer: Option<&str>) -> Observation {
        Observation {
            asn,
            truth: truth.into(),
            answer: answer.map(str::to_string),
        }
    }

    #[test]
    fn ranks_ases_by_misplaced_probes() {
        let ranked = rank([
            seen(64500, "DE", Some("US")),
            seen(64500, "DE", Some("US")),
            seen(64500, "FR", Some("FR")),
            seen(64501, "JP", None),
            seen(64502, "NL", Some("NL")),
            seen(64503, "SG", Some("US")),
        ]);
        let order: Vec<(u32, usize, usize)> = ranked
            .iter()
            .map(|(a, c)| (*a, c.wrong, c.probes))
            .collect();
        assert_eq!(order, [(64500, 2, 3), (64501, 1, 1), (64503, 1, 1)]);
        assert_eq!(ranked[0].1.top_error(), "DE->US x2");
        assert_eq!(ranked[1].1.top_error(), "JP->- x1");
    }

    #[test]
    fn observes_probes_through_both_databases() {
        let countries = database(&[("192.0.2.0/24", country("US"))]);
        let routes = database(&[("192.0.2.0/24", asn(64500))]);
        let probes = [
            Probe {
                id: 1,
                address: "192.0.2.1".parse().unwrap(),
                country: "DE".into(),
                is_anchor: false,
                asn: None,
                auto_located: false,
            },
            Probe {
                id: 2,
                address: "198.51.100.1".parse().unwrap(),
                country: "DE".into(),
                is_anchor: false,
                asn: None,
                auto_located: false,
            },
        ];
        let observations = observe(&probes, &countries, &routes).unwrap();
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].asn, 64500);
        assert_eq!(observations[0].answer.as_deref(), Some("US"));
    }

    #[test]
    fn derives_hosts_from_websites() {
        assert_eq!(
            website_host("http://www.m247global.com").as_deref(),
            Some("m247global.com")
        );
        assert_eq!(
            website_host("https://cloud.oracle.com/").as_deref(),
            Some("cloud.oracle.com")
        );
        assert_eq!(website_host("example").as_deref(), None);
        assert_eq!(website_host("").as_deref(), None);
    }

    #[test]
    fn compares_urls_regardless_of_scheme_and_www() {
        assert_eq!(
            normalized("https://www.Hetzner.com/geofeed.csv"),
            normalized("http://hetzner.com/geofeed.csv")
        );
        assert_ne!(
            normalized("https://a.example/x"),
            normalized("https://b.example/x")
        );
    }

    #[test]
    fn guesses_common_geofeed_locations() {
        let urls = guesses("example.net");
        assert!(urls.contains(&"https://example.net/geofeed.csv".to_string()));
        assert!(urls.contains(&"https://geofeed.example.net/".to_string()));
    }

    #[test]
    fn recognises_geofeeds() {
        let feed: String = (0..12)
            .map(|i| format!("192.0.{i}.0/24,FR,,Paris,\n"))
            .collect();
        assert!(looks_like_geofeed(feed.as_bytes()));
        assert!(!looks_like_geofeed("<html>not a feed</html>\n".as_bytes()));
    }
}
