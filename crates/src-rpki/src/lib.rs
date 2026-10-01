use std::collections::HashMap;
use std::io::Read;
use std::net::IpAddr;

use anyhow::{Context, Result, bail};
use ipnet::IpNet;
use model::{Asn, Vrp};
use serde::Deserialize;

#[derive(Deserialize)]
struct VrpFile {
    roas: Vec<RawVrp>,
}

#[derive(Deserialize)]
struct RawVrp {
    asn: RawAsn,
    prefix: String,
    #[serde(rename = "maxLength")]
    max_length: u8,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawAsn {
    Number(Asn),
    Text(String),
}

impl RawAsn {
    fn to_asn(&self) -> Result<Asn> {
        match self {
            RawAsn::Number(asn) => Ok(*asn),
            RawAsn::Text(text) => {
                let digits = text
                    .strip_prefix("AS")
                    .or_else(|| text.strip_prefix("as"))
                    .unwrap_or(text);
                digits
                    .parse()
                    .with_context(|| format!("invalid ASN {text:?}"))
            }
        }
    }
}

pub fn parse<R: Read>(reader: R) -> Result<Vec<Vrp>> {
    let file: VrpFile = serde_json::from_reader(reader)?;
    file.roas
        .into_iter()
        .map(|raw| {
            let prefix: IpNet = raw
                .prefix
                .parse()
                .with_context(|| format!("invalid prefix {:?}", raw.prefix))?;
            if raw.max_length < prefix.prefix_len() || raw.max_length > prefix.max_prefix_len() {
                bail!("invalid maxLength {} for {prefix}", raw.max_length);
            }
            Ok(Vrp {
                prefix: prefix.trunc(),
                max_length: raw.max_length,
                asn: raw.asn.to_asn()?,
            })
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Validity {
    Valid,
    Invalid,
    NotFound,
}

pub struct Validator {
    index: HashMap<IpNet, Vec<(u8, Asn)>>,
}

impl Validator {
    pub fn new(vrps: &[Vrp]) -> Self {
        let mut index: HashMap<IpNet, Vec<(u8, Asn)>> = HashMap::new();
        for vrp in vrps {
            index
                .entry(vrp.prefix)
                .or_default()
                .push((vrp.max_length, vrp.asn));
        }
        Self { index }
    }

    pub fn validate(&self, route: IpNet, origin: Asn) -> Validity {
        let mut covered = false;
        for length in (0..=route.prefix_len()).rev() {
            let Some(vrps) = self.index.get(&truncate(route, length)) else {
                continue;
            };
            covered = true;
            if vrps
                .iter()
                .any(|&(max, asn)| asn != 0 && asn == origin && route.prefix_len() <= max)
            {
                return Validity::Valid;
            }
        }
        if covered {
            Validity::Invalid
        } else {
            Validity::NotFound
        }
    }
}

fn truncate(network: IpNet, length: u8) -> IpNet {
    let truncated = match network.addr() {
        IpAddr::V4(addr) => IpNet::V4(ipnet::Ipv4Net::new(addr, length).expect("valid length")),
        IpAddr::V6(addr) => IpNet::V6(ipnet::Ipv6Net::new(addr, length).expect("valid length")),
    };
    truncated.trunc()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn net(s: &str) -> IpNet {
        s.parse().unwrap()
    }

    #[test]
    fn parses_rpki_client_and_routinator_formats() {
        let input = r#"{"metadata":{},"roas":[
            {"asn":13335,"prefix":"1.0.0.0/24","maxLength":24,"ta":"apnic","expires":1},
            {"asn":"AS15169","prefix":"2001:4860::/32","maxLength":48,"ta":"arin"}
        ]}"#;
        let vrps = parse(input.as_bytes()).unwrap();
        assert_eq!(
            vrps[0],
            Vrp {
                prefix: net("1.0.0.0/24"),
                max_length: 24,
                asn: 13335
            }
        );
        assert_eq!(vrps[1].asn, 15169);
    }

    #[test]
    fn implements_rfc6811_origin_validation() {
        let validator = Validator::new(&[
            Vrp {
                prefix: net("10.0.0.0/16"),
                max_length: 20,
                asn: 64500,
            },
            Vrp {
                prefix: net("10.1.0.0/16"),
                max_length: 24,
                asn: 0,
            },
            Vrp {
                prefix: net("2001:db8::/32"),
                max_length: 48,
                asn: 64501,
            },
        ]);
        assert_eq!(
            validator.validate(net("10.0.0.0/16"), 64500),
            Validity::Valid
        );
        assert_eq!(
            validator.validate(net("10.0.16.0/20"), 64500),
            Validity::Valid
        );
        assert_eq!(
            validator.validate(net("10.0.16.0/24"), 64500),
            Validity::Invalid
        );
        assert_eq!(
            validator.validate(net("10.0.0.0/16"), 64999),
            Validity::Invalid
        );
        assert_eq!(validator.validate(net("10.1.0.0/24"), 0), Validity::Invalid);
        assert_eq!(
            validator.validate(net("10.2.0.0/16"), 64500),
            Validity::NotFound
        );
        assert_eq!(
            validator.validate(net("2001:db8:1::/48"), 64501),
            Validity::Valid
        );
    }
}
