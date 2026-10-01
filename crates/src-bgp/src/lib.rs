use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, Read};
use std::net::IpAddr;
use std::path::Path;

use anyhow::{Context, Result};
use bgpkit_parser::BgpkitParser;
use bgpkit_parser::models::ElemType;
use flate2::read::MultiGzDecoder;
use ipnet::IpNet;
use model::{Asn, Origin, Route};

pub const IPV4_LENGTHS: std::ops::RangeInclusive<u8> = 8..=24;
pub const IPV6_LENGTHS: std::ops::RangeInclusive<u8> = 16..=48;

#[derive(Default)]
pub struct RibCollector {
    routes: HashMap<IpNet, HashMap<Asn, u32>>,
}

impl RibCollector {
    pub fn add_file(&mut self, path: &Path) -> Result<()> {
        let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
        let reader: Box<dyn Read + Send> = if path.extension().is_some_and(|ext| ext == "gz") {
            Box::new(MultiGzDecoder::new(BufReader::with_capacity(1 << 20, file)))
        } else {
            Box::new(BufReader::with_capacity(1 << 20, file))
        };
        self.add_reader(reader);
        Ok(())
    }

    pub fn add_reader<R: Read>(&mut self, reader: R) {
        let parser = BgpkitParser::from_reader(reader).disable_warnings();
        let mut current: Option<IpNet> = None;
        let mut seen: HashSet<(IpAddr, Asn)> = HashSet::new();
        for route in parser.into_route_iter() {
            if route.elem_type != ElemType::ANNOUNCE {
                continue;
            }
            let prefix = route.prefix.prefix.trunc();
            if !is_routable(prefix) {
                continue;
            }
            let Some(origin) = route
                .as_path
                .as_ref()
                .and_then(|path| path.get_origin_opt())
            else {
                continue;
            };
            if current != Some(prefix) {
                current = Some(prefix);
                seen.clear();
            }
            let origin = origin.to_u32();
            if seen.insert((route.peer_ip, origin)) {
                *self
                    .routes
                    .entry(prefix)
                    .or_default()
                    .entry(origin)
                    .or_default() += 1;
            }
        }
    }

    pub fn into_routes(self) -> Vec<Route> {
        let mut routes: Vec<Route> = self
            .routes
            .into_iter()
            .map(|(prefix, origins)| {
                let mut origins: Vec<Origin> = origins
                    .into_iter()
                    .map(|(asn, peers)| Origin { asn, peers })
                    .collect();
                origins.sort_by(|a, b| b.peers.cmp(&a.peers).then(a.asn.cmp(&b.asn)));
                Route { prefix, origins }
            })
            .collect();
        routes.sort_by(|a, b| a.prefix.cmp(&b.prefix));
        routes
    }
}

pub fn is_routable(prefix: IpNet) -> bool {
    (match prefix {
        IpNet::V4(net) => IPV4_LENGTHS.contains(&net.prefix_len()),
        IpNet::V6(net) => {
            IPV6_LENGTHS.contains(&net.prefix_len()) && net.addr().segments()[0] & 0xe000 == 0x2000
        }
    }) && model::special::find(prefix).is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_global_unicast_lengths_only() {
        assert!(is_routable("1.1.1.0/24".parse().unwrap()));
        assert!(!is_routable("1.1.1.0/25".parse().unwrap()));
        assert!(!is_routable("0.0.0.0/0".parse().unwrap()));
        assert!(!is_routable("2001:db8::/32".parse().unwrap()));
        assert!(!is_routable("2a00:1450::/64".parse().unwrap()));
        assert!(!is_routable("fc00::/16".parse().unwrap()));
        assert!(!is_routable("2002::/16".parse().unwrap()));
        assert!(!is_routable("2001::/32".parse().unwrap()));
        assert!(is_routable("2001:4860::/32".parse().unwrap()));
        assert!(!is_routable("10.0.0.0/8".parse().unwrap()));
        assert!(!is_routable("192.88.99.0/24".parse().unwrap()));
    }
}
