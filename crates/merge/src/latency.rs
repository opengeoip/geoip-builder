use ipnet::IpNet;
use model::Location;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LatencyRule {
    pub max_rtt: f64,
    pub min_ratio: f64,
    pub min_gap: f64,
    pub min_ipv4_length: u8,
    pub min_ipv6_length: u8,
}

impl Default for LatencyRule {
    fn default() -> Self {
        Self {
            max_rtt: 3.0,
            min_ratio: 3.0,
            min_gap: 5.0,
            min_ipv4_length: 22,
            min_ipv6_length: 40,
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct LatencyStats {
    pub measurements: usize,
    pub answered: usize,
    pub located: usize,
    pub too_large: usize,
}

pub struct PrefixPings {
    pub prefix: IpNet,
    pub pings: Vec<(String, f64)>,
}

pub fn locate_by_latency(
    measurements: &[PrefixPings],
    rule: LatencyRule,
) -> (Vec<Location>, LatencyStats) {
    let mut stats = LatencyStats {
        measurements: measurements.len(),
        ..Default::default()
    };
    let mut located = Vec::new();
    for measurement in measurements {
        let min_length = match measurement.prefix {
            IpNet::V4(_) => rule.min_ipv4_length,
            IpNet::V6(_) => rule.min_ipv6_length,
        };
        if measurement.prefix.prefix_len() < min_length {
            stats.too_large += 1;
            continue;
        }
        let Some((country, best)) = measurement
            .pings
            .iter()
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(country, rtt)| (country.as_str(), *rtt))
        else {
            continue;
        };
        stats.answered += 1;
        let other = measurement
            .pings
            .iter()
            .filter(|(c, _)| c != country)
            .map(|(_, rtt)| *rtt)
            .fold(f64::INFINITY, f64::min);
        if best <= rule.max_rtt && other >= best * rule.min_ratio && other >= best + rule.min_gap {
            stats.located += 1;
            located.push(Location {
                network: measurement.prefix,
                country: Some(country.to_string()),
                region: None,
                city: None,
                postal: None,
            });
        }
    }
    (located, stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pings(prefix: &str, pings: &[(&str, f64)]) -> PrefixPings {
        PrefixPings {
            prefix: prefix.parse().unwrap(),
            pings: pings.iter().map(|(c, r)| (c.to_string(), *r)).collect(),
        }
    }

    #[test]
    fn locates_only_unambiguous_nearby_replies() {
        let measurements = vec![
            pings("11.0.0.0/16", &[("JP", 1.2), ("JP", 2.0), ("US", 110.0)]),
            pings("12.0.0.0/16", &[("NL", 1.0), ("DE", 2.5)]),
            pings("13.0.0.0/16", &[("FR", 9.0), ("US", 90.0)]),
            pings("14.0.0.0/16", &[("FR", 2.0)]),
            pings("15.0.0.0/16", &[]),
            pings("16.0.0.0/8", &[("JP", 1.0)]),
        ];
        let (located, stats) = locate_by_latency(&measurements, LatencyRule::default());
        assert!(located.is_empty());
        assert_eq!(stats.too_large, 6);
        let rule = LatencyRule {
            min_ipv4_length: 16,
            ..LatencyRule::default()
        };
        let (located, stats) = locate_by_latency(&measurements, rule);
        let located: Vec<String> = located
            .iter()
            .map(|l| format!("{} {}", l.network, l.country.as_deref().unwrap()))
            .collect();
        assert_eq!(located, ["11.0.0.0/16 JP", "14.0.0.0/16 FR"]);
        assert_eq!(
            stats,
            LatencyStats {
                measurements: 6,
                answered: 4,
                located: 2,
                too_large: 1
            }
        );
    }
}
