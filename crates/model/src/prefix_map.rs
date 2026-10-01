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
}
