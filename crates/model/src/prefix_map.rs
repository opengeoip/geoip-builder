use std::collections::HashMap;
use std::net::IpAddr;

use ipnet::{IpNet, Ipv4Net, Ipv6Net};

#[derive(Debug, Clone)]
pub struct PrefixMap<T> {
    entries: HashMap<IpNet, T>,
}

impl<T> Default for PrefixMap<T> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
}

impl<T> PrefixMap<T> {
    pub fn insert(&mut self, network: IpNet, value: T) -> Option<T> {
        self.entries.insert(network.trunc(), value)
    }

    pub fn entry_or_default(&mut self, network: IpNet) -> &mut T
    where
        T: Default,
    {
        self.entries.entry(network.trunc()).or_default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn covering(&self, network: IpNet) -> impl Iterator<Item = (IpNet, &T)> {
        (0..=network.prefix_len()).rev().filter_map(move |length| {
            let candidate = truncate(network, length);
            self.entries.get(&candidate).map(|value| (candidate, value))
        })
    }

    pub fn longest_match(&self, network: IpNet) -> Option<(IpNet, &T)> {
        self.covering(network).next()
    }

    pub fn iter(&self) -> impl Iterator<Item = (IpNet, &T)> {
        self.entries
            .iter()
            .map(|(network, value)| (*network, value))
    }
}

impl<T> FromIterator<(IpNet, T)> for PrefixMap<T> {
    fn from_iter<I: IntoIterator<Item = (IpNet, T)>>(iter: I) -> Self {
        let mut map = PrefixMap::default();
        for (network, value) in iter {
            map.insert(network, value);
        }
        map
    }
}

fn block_end(start: u128, size: u32) -> u128 {
    if size >= 128 {
        u128::MAX
    } else {
        start | ((1u128 << size) - 1)
    }
}

pub fn range_to_networks(start: IpAddr, end: IpAddr) -> Option<Vec<IpNet>> {
    let (mut current, last, bits) = match (start, end) {
        (IpAddr::V4(a), IpAddr::V4(b)) => (u128::from(u32::from(a)), u128::from(u32::from(b)), 32),
        (IpAddr::V6(a), IpAddr::V6(b)) => (u128::from(a), u128::from(b), 128),
        _ => return None,
    };
    if current > last {
        return None;
    }
    let mut networks = Vec::new();
    loop {
        let mut size = current.trailing_zeros().min(bits);
        while block_end(current, size) > last {
            size -= 1;
        }
        let length = (bits - size) as u8;
        networks.push(match start {
            IpAddr::V4(_) => IpNet::V4(Ipv4Net::new((current as u32).into(), length).ok()?),
            IpAddr::V6(_) => IpNet::V6(Ipv6Net::new(current.into(), length).ok()?),
        });
        let end = block_end(current, size);
        if end >= last {
            return Some(networks);
        }
        current = end + 1;
    }
}

pub fn truncate(network: IpNet, length: u8) -> IpNet {
    match network.addr() {
        IpAddr::V4(addr) => IpNet::V4(Ipv4Net::new(addr, length).expect("valid length").trunc()),
        IpAddr::V6(addr) => IpNet::V6(Ipv6Net::new(addr, length).expect("valid length").trunc()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn net(s: &str) -> IpNet {
        s.parse().unwrap()
    }

    #[test]
    fn finds_the_longest_covering_prefix() {
        let map: PrefixMap<&str> = [(net("10.0.0.0/8"), "a"), (net("10.1.0.0/16"), "b")]
            .into_iter()
            .collect();
        assert_eq!(
            map.longest_match(net("10.1.2.0/24")),
            Some((net("10.1.0.0/16"), &"b"))
        );
        assert_eq!(
            map.longest_match(net("10.2.0.0/24")),
            Some((net("10.0.0.0/8"), &"a"))
        );
        assert_eq!(map.longest_match(net("10.0.0.0/7")), None);
        assert_eq!(map.covering(net("10.1.0.0/16")).count(), 2);
    }

    #[test]
    fn splits_address_ranges() {
        let split = |a: &str, b: &str| -> Vec<String> {
            range_to_networks(a.parse().unwrap(), b.parse().unwrap())
                .unwrap()
                .iter()
                .map(ToString::to_string)
                .collect()
        };
        assert_eq!(
            split("192.0.2.0", "192.0.3.127"),
            ["192.0.2.0/24", "192.0.3.0/25"]
        );
        assert_eq!(split("0.0.0.0", "255.255.255.255"), ["0.0.0.0/0"]);
        assert_eq!(
            split("2631:7000::", "2631:700f:ffff:ffff:ffff:ffff:ffff:ffff"),
            ["2631:7000::/28"]
        );
        assert_eq!(
            split("::", "ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff"),
            ["::/0"]
        );
        assert_eq!(
            split("10.0.0.1", "10.0.0.2"),
            ["10.0.0.1/32", "10.0.0.2/32"]
        );
        assert!(
            range_to_networks("10.0.0.2".parse().unwrap(), "10.0.0.1".parse().unwrap()).is_none()
        );
    }
}
