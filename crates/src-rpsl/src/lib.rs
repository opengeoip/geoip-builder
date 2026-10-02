use std::io::BufRead;
use std::net::{IpAddr, Ipv4Addr};

use anyhow::Result;
use ipnet::IpNet;
use model::{Assignment, GeofeedRef, Registry, country_code};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub objects: usize,
    pub countries: usize,
    pub references: usize,
    pub rejected: usize,
}

#[derive(Debug, Default)]
pub struct Parsed {
    pub references: Vec<GeofeedRef>,
    pub assignments: Vec<Assignment>,
    pub stats: Stats,
}

#[derive(Default)]
struct Object {
    networks: Option<String>,
    country: Option<String>,
    geofeed: Option<String>,
    remark: Option<String>,
}

impl Object {
    fn finish(&mut self, registry: Registry, out: &mut Parsed) {
        let object = std::mem::take(self);
        let Some(networks) = object.networks else {
            return;
        };
        out.stats.objects += 1;
        let Some(networks) = parse_networks(&networks) else {
            return;
        };
        if let Some(country) = object.country.as_deref().and_then(country_code) {
            out.stats.countries += 1;
            out.assignments
                .extend(networks.iter().map(|&network| Assignment {
                    network,
                    country,
                    registry,
                }));
        }
        let Some(url) = object.geofeed.or(object.remark) else {
            return;
        };
        if is_web_url(&url) {
            out.stats.references += 1;
            out.references
                .extend(networks.into_iter().map(|network| GeofeedRef {
                    network,
                    url: url.clone(),
                }));
        } else {
            out.stats.rejected += 1;
        }
    }
}

pub fn parse<R: BufRead>(mut reader: R, registry: Registry) -> Result<Parsed> {
    let mut parsed = Parsed::default();
    let mut object = Object::default();
    let mut buffer = Vec::new();
    while reader.read_until(b'\n', &mut buffer)? > 0 {
        let line = String::from_utf8_lossy(&buffer);
        let line = line.trim_end();
        if line.is_empty() {
            object.finish(registry, &mut parsed);
        } else if !line.starts_with([' ', '\t', '+', '#', '%'])
            && let Some((key, value)) = line.split_once(':')
        {
            let value = value.trim();
            match key.to_ascii_lowercase().as_str() {
                "inetnum" | "inet6num" => object.networks = Some(value.to_string()),
                "country" if object.country.is_none() => object.country = Some(value.to_string()),
                "geofeed" if object.geofeed.is_none() => object.geofeed = Some(value.to_string()),
                "remarks" if object.remark.is_none() => {
                    object.remark = remark_url(value).map(str::to_string);
                }
                _ => {}
            }
        }
        buffer.clear();
    }
    object.finish(registry, &mut parsed);
    Ok(parsed)
}

fn starts_with_ignore_case(value: &str, prefix: &str) -> bool {
    value
        .get(..prefix.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
}

pub fn remark_url(value: &str) -> Option<&str> {
    let mut rest = value.trim().trim_start_matches(['|', ' ', '\t']);
    for prefix in ["remarks:", "comment:"] {
        if starts_with_ignore_case(rest, prefix) {
            rest = rest[prefix.len()..].trim_start();
        }
    }
    const KEYWORD: &str = "geofeed";
    if !starts_with_ignore_case(rest, KEYWORD) {
        return None;
    }
    let rest = &rest[KEYWORD.len()..];
    let url = rest.strip_prefix(':').unwrap_or(rest).trim();
    let url = url.split_whitespace().next()?;
    (rest.starts_with([':', ' ', '\t'])).then_some(url)
}

pub fn is_web_url(url: &str) -> bool {
    ["https://", "http://"]
        .iter()
        .any(|scheme| url.len() > scheme.len() && starts_with_ignore_case(url, scheme))
        && !url.contains(char::is_whitespace)
}

pub fn parse_networks(value: &str) -> Option<Vec<IpNet>> {
    if let Some((start, end)) = value.split_once('-') {
        let start: Ipv4Addr = start.trim().parse().ok()?;
        let end: Ipv4Addr = end.trim().parse().ok()?;
        let count = u64::from(u32::from(end)).checked_sub(u64::from(u32::from(start)))? + 1;
        let networks = src_delegated::ipv4_range_to_networks(start, count).ok()?;
        return Some(networks.into_iter().map(IpNet::V4).collect());
    }
    let (address, length) = value.split_once('/')?;
    let length: u8 = length.trim().parse().ok()?;
    let address: IpAddr = if address.contains(':') {
        address.parse().ok()?
    } else {
        let mut octets: Vec<&str> = address.split('.').collect();
        if octets.is_empty() || octets.len() > 4 {
            return None;
        }
        octets.resize(4, "0");
        octets.join(".").parse().ok()?
    };
    Some(vec![IpNet::new(address, length).ok()?.trunc()])
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
inetnum:        195.167.179.160 - 195.167.179.167
netname:        EXAMPLE
country:        gb
country:        FR
geofeed:        https://static.example.uk/geofeed.csv
source:         RIPE

inetnum:    45.4.200/22
status:     allocated
remarks:    Geofeed https://fibramax.example/geofeeds.csv
source:     LACNIC

inet6num:       2001:db8::/32
remarks:        geofeed http://plain.example/feed.csv

inet6num:       2001:db9::/32
remarks:        geofeed ftp://old.example/feed.csv

inetnum:        10.0.0.0 - 10.0.0.255
country:        EU
remarks:        hello

aut-num:        AS64500
geofeed:        https://ignored.example/feed.csv
";

    fn net(s: &str) -> IpNet {
        s.parse().unwrap()
    }

    #[test]
    fn extracts_geofeed_references() {
        let parsed = parse(SAMPLE.as_bytes(), Registry::RipeNcc).unwrap();
        assert_eq!(
            parsed.assignments,
            [Assignment {
                network: net("195.167.179.160/29"),
                country: *b"GB",
                registry: Registry::RipeNcc,
            }]
        );
        assert_eq!(
            parsed.references,
            [
                GeofeedRef {
                    network: net("195.167.179.160/29"),
                    url: "https://static.example.uk/geofeed.csv".into()
                },
                GeofeedRef {
                    network: net("45.4.200.0/22"),
                    url: "https://fibramax.example/geofeeds.csv".into()
                },
                GeofeedRef {
                    network: net("2001:db8::/32"),
                    url: "http://plain.example/feed.csv".into()
                },
            ]
        );
        assert_eq!(
            parsed.stats,
            Stats {
                objects: 5,
                countries: 1,
                references: 3,
                rejected: 1
            }
        );
    }

    #[test]
    fn accepts_the_remark_spellings_seen_in_the_wild() {
        for remark in [
            "Geofeed https://a.example/g.csv",
            "geofeed: https://a.example/g.csv",
            "GeoFeed:https://a.example/g.csv",
            "Comment: Geofeed https://a.example/g.csv",
            "remarks: Geofeed https://a.example/g.csv",
            "| Geofeed: https://a.example/g.csv",
        ] {
            assert_eq!(
                remark_url(remark),
                Some("https://a.example/g.csv"),
                "{remark}"
            );
        }
        for remark in [
            "https://a.example/g.csv",
            "geofeeds are nice",
            "Geofeedhttps://a.example",
            "ANR Si\u{e8}ge",
            "G\u{e9}ofeed https://a.example/g.csv",
            "geo",
        ] {
            assert_eq!(remark_url(remark), None, "{remark}");
        }
    }

    #[test]
    fn splits_unaligned_ranges() {
        assert_eq!(
            parse_networks("192.0.2.0 - 192.0.3.127").unwrap(),
            [net("192.0.2.0/24"), net("192.0.3.0/25")]
        );
        assert_eq!(parse_networks("200.10/16").unwrap(), [net("200.10.0.0/16")]);
        assert!(parse_networks("garbage").is_none());
    }
}
