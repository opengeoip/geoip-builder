use std::io::BufRead;
use std::net::{IpAddr, Ipv4Addr};

use anyhow::Result;
use ipnet::IpNet;
use model::GeofeedRef;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub objects: usize,
    pub references: usize,
    pub rejected: usize,
}

#[derive(Default)]
struct Object {
    networks: Option<String>,
    geofeed: Option<String>,
    remark: Option<String>,
}

impl Object {
    fn finish(&mut self, out: &mut Vec<GeofeedRef>, stats: &mut Stats) {
        let object = std::mem::take(self);
        let Some(networks) = object.networks else {
            return;
        };
        stats.objects += 1;
        let Some(url) = object.geofeed.or(object.remark) else {
            return;
        };
        match (parse_networks(&networks), is_https(&url)) {
            (Some(networks), true) => {
                stats.references += 1;
                out.extend(networks.into_iter().map(|network| GeofeedRef {
                    network,
                    url: url.clone(),
                }));
            }
            _ => stats.rejected += 1,
        }
    }
}

pub fn parse<R: BufRead>(mut reader: R) -> Result<(Vec<GeofeedRef>, Stats)> {
    let mut references = Vec::new();
    let mut stats = Stats::default();
    let mut object = Object::default();
    let mut buffer = Vec::new();
    while reader.read_until(b'\n', &mut buffer)? > 0 {
        let line = String::from_utf8_lossy(&buffer);
        let line = line.trim_end();
        if line.is_empty() {
            object.finish(&mut references, &mut stats);
        } else if !line.starts_with([' ', '\t', '+', '#', '%'])
            && let Some((key, value)) = line.split_once(':')
        {
            let value = value.trim();
            match key.to_ascii_lowercase().as_str() {
                "inetnum" | "inet6num" => object.networks = Some(value.to_string()),
                "geofeed" if object.geofeed.is_none() => object.geofeed = Some(value.to_string()),
                "remarks" if object.remark.is_none() => {
                    object.remark = remark_url(value).map(str::to_string);
                }
                _ => {}
            }
        }
        buffer.clear();
    }
    object.finish(&mut references, &mut stats);
    Ok((references, stats))
}

fn remark_url(value: &str) -> Option<&str> {
    let (keyword, url) = value.split_once(char::is_whitespace)?;
    keyword.eq_ignore_ascii_case("geofeed").then(|| url.trim())
}

fn is_https(url: &str) -> bool {
    url.len() > 8 && url[..8].eq_ignore_ascii_case("https://") && !url.contains(char::is_whitespace)
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
country:        GB
geofeed:        https://static.example.uk/geofeed.csv
source:         RIPE

inetnum:    45.4.200/22
status:     allocated
remarks:    Geofeed https://fibramax.example/geofeeds.csv
source:     LACNIC

inet6num:       2001:db8::/32
remarks:        geofeed http://insecure.example/feed.csv

inetnum:        10.0.0.0 - 10.0.0.255
remarks:        hello

aut-num:        AS64500
geofeed:        https://ignored.example/feed.csv
";

    fn net(s: &str) -> IpNet {
        s.parse().unwrap()
    }

    #[test]
    fn extracts_geofeed_references() {
        let (references, stats) = parse(SAMPLE.as_bytes()).unwrap();
        assert_eq!(
            references,
            [
                GeofeedRef {
                    network: net("195.167.179.160/29"),
                    url: "https://static.example.uk/geofeed.csv".into()
                },
                GeofeedRef {
                    network: net("45.4.200.0/22"),
                    url: "https://fibramax.example/geofeeds.csv".into()
                },
            ]
        );
        assert_eq!(
            stats,
            Stats {
                objects: 4,
                references: 2,
                rejected: 1
            }
        );
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
