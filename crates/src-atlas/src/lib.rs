use std::collections::HashSet;
use std::io::{BufRead, Read};
use std::net::IpAddr;

use anyhow::Result;
use bzip2::read::MultiBzDecoder;
use serde::Deserialize;

const CONNECTED: u8 = 1;
const SYSTEM_TAG: &str = "system-";
const AUTO_LOCATED: [&str; 2] = ["system-auto-geoip-country", "system-auto-geoip-city"];

#[derive(Deserialize)]
struct Archive {
    objects: Vec<RawProbe>,
}

#[derive(Deserialize)]
struct RawProbe {
    id: u32,
    status: u8,
    is_anchor: bool,
    country_code: Option<String>,
    address_v4: Option<IpAddr>,
    address_v6: Option<IpAddr>,
    asn_v4: Option<u32>,
    asn_v6: Option<u32>,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Probe {
    pub id: u32,
    pub address: IpAddr,
    pub country: String,
    pub is_anchor: bool,
    pub asn: Option<u32>,
    pub auto_located: bool,
    pub tags: Vec<String>,
}

pub fn parse<R: Read>(reader: R) -> Result<Vec<Probe>> {
    parse_json(MultiBzDecoder::new(reader))
}

pub fn parse_ids<R: BufRead>(reader: R) -> Result<HashSet<u32>> {
    let mut ids = HashSet::new();
    for line in reader.lines() {
        let line = line?;
        let field = line
            .split([',', ' ', '\t'])
            .next()
            .unwrap_or_default()
            .trim();
        if let Ok(id) = field.parse() {
            ids.insert(id);
        }
    }
    Ok(ids)
}

pub fn parse_json<R: Read>(reader: R) -> Result<Vec<Probe>> {
    let archive: Archive = serde_json::from_reader(reader)?;
    let mut probes = Vec::new();
    for raw in archive.objects {
        let Some(country) = raw.country_code.filter(|c| c.len() == 2) else {
            continue;
        };
        if raw.status != CONNECTED {
            continue;
        }
        let auto_located = raw.tags.iter().any(|t| AUTO_LOCATED.contains(&t.as_str()));
        let tags: Vec<String> = raw
            .tags
            .into_iter()
            .filter(|t| !t.starts_with(SYSTEM_TAG))
            .collect();
        for (address, asn) in [(raw.address_v4, raw.asn_v4), (raw.address_v6, raw.asn_v6)] {
            let Some(address) = address else {
                continue;
            };
            probes.push(Probe {
                id: raw.id,
                address,
                country: country.to_ascii_uppercase(),
                is_anchor: raw.is_anchor,
                asn,
                auto_located,
                tags: tags.clone(),
            });
        }
    }
    Ok(probes)
}

pub fn archive_url(year: i64, month: u32, day: u32) -> String {
    format!(
        "https://ftp.ripe.net/ripe/atlas/probes/archive/{year:04}/{month:02}/{year:04}{month:02}{day:02}.json.bz2"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_connected_probes_with_public_addresses() {
        let input = r#"{"meta":{},"objects":[
            {"id":43,"status":1,"is_anchor":false,"country_code":"FR","address_v4":"77.95.64.208","address_v6":"2a03:9180:1:20::1","asn_v4":3215,"asn_v6":5410,"tags":["home","system-auto-geoip-city"]},
            {"id":2,"status":3,"is_anchor":false,"country_code":"RS","address_v4":"1.2.3.4","address_v6":null},
            {"id":7,"status":1,"is_anchor":true,"country_code":"de","address_v4":null,"address_v6":null},
            {"id":9,"status":1,"is_anchor":true,"country_code":"nl","address_v4":"193.0.0.1","address_v6":null}
        ]}"#;
        let probes = parse_json(input.as_bytes()).unwrap();
        let summary: Vec<String> = probes
            .iter()
            .map(|p| {
                format!(
                    "{} {} {} {} {:?} {}",
                    p.id, p.address, p.country, p.is_anchor, p.asn, p.auto_located
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                "43 77.95.64.208 FR false Some(3215) true",
                "43 2a03:9180:1:20::1 FR false Some(5410) true",
                "9 193.0.0.1 NL true None false"
            ]
        );
        assert_eq!(probes[0].tags, ["home"]);
    }

    #[test]
    fn reads_probe_id_lists() {
        let ids = parse_ids("1004033\n1000963,true,false\nprobe_id\n\n".as_bytes()).unwrap();
        assert_eq!(ids, HashSet::from([1004033, 1000963]));
    }
}
