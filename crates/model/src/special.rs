use std::sync::LazyLock;

use ipnet::IpNet;

pub struct SpecialRange {
    pub network: IpNet,
    pub name: &'static str,
}

const RANGES: &[(&str, &str)] = &[
    ("0.0.0.0/8", "This network"),
    ("10.0.0.0/8", "Private-Use"),
    ("100.64.0.0/10", "Shared Address Space"),
    ("127.0.0.0/8", "Loopback"),
    ("169.254.0.0/16", "Link Local"),
    ("172.16.0.0/12", "Private-Use"),
    ("192.0.0.0/24", "IETF Protocol Assignments"),
    ("192.0.2.0/24", "Documentation (TEST-NET-1)"),
    ("192.88.99.0/24", "Deprecated 6to4 Relay Anycast"),
    ("192.168.0.0/16", "Private-Use"),
    ("198.18.0.0/15", "Benchmarking"),
    ("198.51.100.0/24", "Documentation (TEST-NET-2)"),
    ("203.0.113.0/24", "Documentation (TEST-NET-3)"),
    ("224.0.0.0/4", "Multicast"),
    ("240.0.0.0/4", "Reserved"),
    ("64:ff9b:1::/48", "Local-Use IPv4/IPv6 Translation"),
    ("100::/64", "Discard-Only"),
    ("2001::/32", "Teredo"),
    ("2001:2::/48", "Benchmarking"),
    ("2001:10::/28", "Deprecated ORCHID"),
    ("2001:db8::/32", "Documentation"),
    ("2002::/16", "6to4"),
    ("3fff::/20", "Documentation"),
    ("5f00::/16", "Segment Routing SIDs"),
    ("fc00::/7", "Unique-Local"),
    ("fe80::/10", "Link-Local Unicast"),
    ("ff00::/8", "Multicast"),
];

static SPECIAL: LazyLock<Vec<SpecialRange>> = LazyLock::new(|| {
    RANGES
        .iter()
        .map(|(network, name)| SpecialRange {
            network: network.parse().expect("valid special-purpose range"),
            name,
        })
        .collect()
});

pub fn ranges() -> &'static [SpecialRange] {
    &SPECIAL
}

pub fn find(network: IpNet) -> Option<&'static SpecialRange> {
    SPECIAL
        .iter()
        .find(|range| range.network.contains(&network))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn net(s: &str) -> IpNet {
        s.parse().unwrap()
    }

    #[test]
    fn finds_covering_special_ranges() {
        assert_eq!(find(net("10.1.2.0/24")).unwrap().name, "Private-Use");
        assert_eq!(find(net("2002:808:808::/48")).unwrap().name, "6to4");
        assert!(find(net("8.8.8.0/24")).is_none());
        assert!(find(net("192.0.0.0/8")).is_none());
        assert!(find(net("2001:4860::/32")).is_none());
    }
}
