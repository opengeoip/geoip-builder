use std::collections::HashSet;
use std::io::{BufRead, Read};
use std::net::IpAddr;

use anyhow::Result;
use bzip2::read::MultiBzDecoder;
use serde::Deserialize;

const CONNECTED: u8 = 1;

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
    latitude: Option<f64>,
    longitude: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProbeSite {
    pub id: u32,
    pub country: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Probe {
    pub id: u32,
    pub address: IpAddr,
    pub country: String,
    pub is_anchor: bool,
}

pub fn parse<R: Read>(reader: R) -> Result<Vec<Probe>> {
    parse_json(MultiBzDecoder::new(reader))
}

pub fn parse_sites<R: Read>(reader: R) -> Result<Vec<ProbeSite>> {
    let archive: Archive = serde_json::from_reader(MultiBzDecoder::new(reader))?;
    Ok(archive
        .objects
        .into_iter()
        .filter_map(|raw| {
            let country = raw
                .country_code
                .filter(|c| c.len() == 2)?
                .to_ascii_uppercase();
            Some(ProbeSite {
                id: raw.id,
                country,
                latitude: raw.latitude,
                longitude: raw.longitude,
            })
        })
        .collect())
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

#[derive(Deserialize)]
pub struct MeasurementPage {
    pub next: Option<String>,
    pub results: Vec<Measurement>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, serde::Serialize)]
pub struct Measurement {
    pub id: u64,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub target: Option<String>,
}

#[derive(Deserialize)]
struct RawPing {
    prb_id: u32,
    min: f64,
}

pub fn parse_ping_results<R: Read>(reader: R) -> Result<Vec<(u32, f64)>> {
    let results: Vec<RawPing> = serde_json::from_reader(reader)?;
    Ok(results
        .into_iter()
        .filter(|r| r.min > 0.0)
        .map(|r| (r.prb_id, r.min))
        .collect())
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
        for address in [raw.address_v4, raw.address_v6].into_iter().flatten() {
            probes.push(Probe {
                id: raw.id,
                address,
                country: country.to_ascii_uppercase(),
                is_anchor: raw.is_anchor,
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

pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_connected_probes_with_public_addresses() {
        let input = r#"{"meta":{},"objects":[
            {"id":43,"status":1,"is_anchor":false,"country_code":"FR","address_v4":"77.95.64.208","address_v6":"2a03:9180:1:20::1"},
            {"id":2,"status":3,"is_anchor":false,"country_code":"RS","address_v4":"1.2.3.4","address_v6":null},
            {"id":7,"status":1,"is_anchor":true,"country_code":"de","address_v4":null,"address_v6":null},
            {"id":9,"status":1,"is_anchor":true,"country_code":"nl","address_v4":"193.0.0.1","address_v6":null}
        ]}"#;
        let probes = parse_json(input.as_bytes()).unwrap();
        let summary: Vec<String> = probes
            .iter()
            .map(|p| format!("{} {} {} {}", p.id, p.address, p.country, p.is_anchor))
            .collect();
        assert_eq!(
            summary,
            [
                "43 77.95.64.208 FR false",
                "43 2a03:9180:1:20::1 FR false",
                "9 193.0.0.1 NL true"
            ]
        );
    }

    #[test]
    fn reads_probe_id_lists_and_ping_results() {
        let ids = parse_ids("1004033\n1000963,true,false\nprobe_id\n\n".as_bytes()).unwrap();
        assert_eq!(ids, HashSet::from([1004033, 1000963]));
        let pings = parse_ping_results(
            r#"[{"prb_id":6380,"min":-1,"avg":-1},{"prb_id":7,"min":1.25,"avg":1.3}]"#.as_bytes(),
        )
        .unwrap();
        assert_eq!(pings, [(7, 1.25)]);
    }

    #[test]
    fn converts_days_to_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_726), (2026, 9, 30));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
    }
}
