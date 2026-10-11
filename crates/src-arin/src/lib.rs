use std::io::Read;
use std::net::IpAddr;

use anyhow::Result;
use model::{GeofeedRef, range_to_networks};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct NetRange {
    pub handle: String,
    #[serde(rename = "startAddress")]
    pub start: IpAddr,
    #[serde(rename = "endAddress")]
    pub end: IpAddr,
    #[serde(default)]
    pub remarks: Vec<String>,
}

impl NetRange {
    pub fn geofeed(&self) -> Option<&str> {
        geofeed_in(self.remarks.iter().map(String::as_str))
    }
}

fn geofeed_in<'a>(lines: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    lines
        .filter_map(|line| src_rpsl::remark_url(line.trim()))
        .find(|url| src_rpsl::is_web_url(url))
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub objects: usize,
    pub references: usize,
    pub rejected: usize,
}

pub fn parse<R: Read>(reader: R) -> Result<(Vec<NetRange>, Vec<GeofeedRef>, Stats)> {
    let ranges: Vec<NetRange> = serde_json::from_reader(reader)?;
    let mut stats = Stats {
        objects: ranges.len(),
        ..Default::default()
    };
    let mut references = Vec::new();
    for range in &ranges {
        match (range.geofeed(), range_to_networks(range.start, range.end)) {
            (Some(url), Some(networks)) => {
                stats.references += 1;
                references.extend(networks.into_iter().map(|network| GeofeedRef {
                    network,
                    url: url.to_string(),
                }));
            }
            _ => stats.rejected += 1,
        }
    }
    Ok((ranges, references, stats))
}

#[derive(Deserialize)]
struct RdapResponse {
    handle: String,
    #[serde(rename = "startAddress")]
    start: IpAddr,
    #[serde(rename = "endAddress")]
    end: IpAddr,
    #[serde(default)]
    remarks: Vec<RdapRemark>,
    #[serde(default)]
    links: Vec<RdapLink>,
}

#[derive(Deserialize)]
struct RdapRemark {
    #[serde(default)]
    description: Vec<String>,
}

#[derive(Deserialize)]
struct RdapLink {
    #[serde(default)]
    rel: String,
    href: String,
}

pub struct RdapNetwork {
    pub handle: String,
    pub start: IpAddr,
    pub end: IpAddr,
    pub geofeed: Option<String>,
    pub up: Option<String>,
}

pub fn rdap_network<R: Read>(reader: R) -> Result<RdapNetwork> {
    let response: RdapResponse = serde_json::from_reader(reader)?;
    let geofeed = geofeed_in(
        response
            .remarks
            .iter()
            .flat_map(|r| r.description.iter().map(String::as_str)),
    )
    .map(str::to_string);
    let up = response
        .links
        .into_iter()
        .find(|link| link.rel == "up")
        .map(|link| link.href);
    Ok(RdapNetwork {
        handle: response.handle,
        start: response.start,
        end: response.end,
        geofeed,
        up,
    })
}

pub fn rdap_url(range: &NetRange) -> String {
    match range_to_networks(range.start, range.end).and_then(|n| n.into_iter().next()) {
        Some(network) => format!("https://rdap.arin.net/registry/ip/{network}"),
        None => format!("https://rdap.arin.net/registry/ip/{}", range.start),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_precompiled_netranges() {
        let input = r#"[
            {"handle":"NET-223-165-96-0-1","startAddress":"223.165.96.0","endAddress":"223.165.111.255","remarks":["Geofeed https://gpcom.example/geofeed.csv"]},
            {"handle":"NET6-2631-7000-1","startAddress":"2631:7000::","endAddress":"2631:700f:ffff:ffff:ffff:ffff:ffff:ffff","remarks":["hello","geofeed https://fiber.example/g.csv"]},
            {"handle":"NET-1","startAddress":"192.0.2.0","endAddress":"192.0.2.255","remarks":["Geofeed ftp://old.example/g.csv"]}
        ]"#;
        let (ranges, references, stats) = parse(input.as_bytes()).unwrap();
        assert_eq!(ranges.len(), 3);
        let references: Vec<String> = references
            .iter()
            .map(|r| format!("{} {}", r.network, r.url))
            .collect();
        assert_eq!(
            references,
            [
                "223.165.96.0/20 https://gpcom.example/geofeed.csv",
                "2631:7000::/28 https://fiber.example/g.csv"
            ]
        );
        assert_eq!(
            stats,
            Stats {
                objects: 3,
                references: 2,
                rejected: 1
            }
        );
    }

    #[test]
    fn reads_rdap_networks() {
        let input = r#"{"handle":"NET-1","startAddress":"192.0.2.0","endAddress":"192.0.2.255","remarks":[{"title":"Registration Comments","description":["Geofeed https://gpcom.example/geofeed.csv"]}],"links":[{"rel":"self","href":"https://rdap.example/ip/192.0.2.0"},{"rel":"up","href":"https://rdap.example/ip/192.0.0.0/22"}]}"#;
        let network = rdap_network(input.as_bytes()).unwrap();
        assert_eq!(network.handle, "NET-1");
        assert_eq!(
            network.geofeed.as_deref(),
            Some("https://gpcom.example/geofeed.csv")
        );
        assert_eq!(
            network.up.as_deref(),
            Some("https://rdap.example/ip/192.0.0.0/22")
        );
        let bare = r#"{"handle":"NET-2","startAddress":"192.0.2.0","endAddress":"192.0.2.255"}"#;
        let network = rdap_network(bare.as_bytes()).unwrap();
        assert_eq!((network.geofeed, network.up), (None, None));
    }

    #[test]
    fn queries_the_first_block_of_the_range() {
        let range = NetRange {
            handle: "NET-198-145-66-0-1".to_string(),
            start: "198.145.66.0".parse().unwrap(),
            end: "198.145.103.255".parse().unwrap(),
            remarks: Vec::new(),
        };
        assert_eq!(
            rdap_url(&range),
            "https://rdap.arin.net/registry/ip/198.145.66.0/23"
        );
    }
}
