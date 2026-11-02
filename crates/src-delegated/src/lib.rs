use std::io::BufRead;
use std::net::{Ipv4Addr, Ipv6Addr};

use anyhow::{Context, Result, bail};
use ipnet::{IpNet, Ipv4Net, Ipv6Net};
use model::{Country, Delegation, Registry};

pub fn parse<R: BufRead>(reader: R) -> Result<Vec<Delegation>> {
    let mut delegations = Vec::new();
    for (index, line) in reader.lines().enumerate() {
        let line = line?;
        parse_line(&line, &mut delegations)
            .with_context(|| format!("line {}: {line}", index + 1))?;
    }
    Ok(delegations)
}

fn parse_line(line: &str, out: &mut Vec<Delegation>) -> Result<()> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return Ok(());
    }
    let fields: Vec<&str> = line.split('|').collect();
    if fields.len() < 7 || fields[0].starts_with(|c: char| c.is_ascii_digit()) {
        return Ok(());
    }
    if fields[5] == "summary" {
        return Ok(());
    }
    let status = fields[6];
    if status != "allocated" && status != "assigned" {
        return Ok(());
    }
    let country = Country::parse(fields[1]);
    let registry: Registry = fields[0].parse()?;
    match fields[2] {
        "ipv4" => {
            let start: Ipv4Addr = fields[3].parse()?;
            let count: u64 = fields[4].parse()?;
            for network in ipv4_range_to_networks(start, count)? {
                out.push(Delegation {
                    network: IpNet::V4(network),
                    country,
                    registry,
                });
            }
        }
        "ipv6" => {
            let start: Ipv6Addr = fields[3].parse()?;
            let length: u8 = fields[4].parse()?;
            let network = Ipv6Net::new(start, length)?.trunc();
            out.push(Delegation {
                network: IpNet::V6(network),
                country,
                registry,
            });
        }
        _ => {}
    }
    Ok(())
}

pub fn ipv4_range_to_networks(start: Ipv4Addr, count: u64) -> Result<Vec<Ipv4Net>> {
    let mut current = u64::from(u32::from(start));
    let end = current + count;
    if count == 0 || end > 1 << 32 {
        bail!("invalid IPv4 range {start} + {count}");
    }
    let mut networks = Vec::new();
    while current < end {
        let alignment = if current == 0 {
            32
        } else {
            current.trailing_zeros().min(32)
        };
        let mut size = alignment;
        while (1u64 << size) > end - current {
            size -= 1;
        }
        networks.push(Ipv4Net::new(
            Ipv4Addr::from(current as u32),
            32 - size as u8,
        )?);
        current += 1 << size;
    }
    Ok(networks)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
2|ripencc|1790805599|260822|19700101|20260930|+0200
ripencc|*|ipv4|*|100901|summary
ripencc|PS|ipv4|1.178.112.0|4096|20071126|allocated|bdc40251
ripencc|FR|ipv4|2.0.0.0|1536|20100712|allocated|aaa
ripencc|DE|ipv6|2001:608::|32|19990812|allocated|bbb
ripencc|EU|ipv4|139.191.0.0|65536|19930125|assigned|ddd
ripencc||ipv4|5.0.0.0|256||available|
ripencc|NL|asn|1101|1|19930901|allocated|ccc
ripencc|ZZ|ipv4|6.0.0.0|256|20100712|reserved|
";

    #[test]
    fn parses_allocations_and_keeps_blocks_without_a_country() {
        let delegations = parse(SAMPLE.as_bytes()).unwrap();
        let networks: Vec<String> = delegations
            .iter()
            .map(|d| format!("{} {}", d.network, d.country.map_or("-", Country::as_str)))
            .collect();
        assert_eq!(
            networks,
            [
                "1.178.112.0/20 PS",
                "2.0.0.0/22 FR",
                "2.0.4.0/23 FR",
                "2001:608::/32 DE",
                "139.191.0.0/16 -"
            ]
        );
        assert!(delegations.iter().all(|d| d.registry == Registry::RipeNcc));
    }

    #[test]
    fn splits_unaligned_ranges() {
        let networks = ipv4_range_to_networks("10.0.1.0".parse().unwrap(), 768).unwrap();
        let networks: Vec<String> = networks.iter().map(ToString::to_string).collect();
        assert_eq!(networks, ["10.0.1.0/24", "10.0.2.0/23"]);
    }

    #[test]
    fn covers_the_whole_space() {
        let networks = ipv4_range_to_networks("0.0.0.0".parse().unwrap(), 1 << 32).unwrap();
        assert_eq!(networks, ["0.0.0.0/0".parse::<Ipv4Net>().unwrap()]);
    }
}
