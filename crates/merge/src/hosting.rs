use std::collections::{HashMap, HashSet};

use ipnet::IpNet;
use mmdb_writer::{Value, Writer};
use model::Asn;

use crate::{SelectedRoute, clear_special_ranges, writer};

pub const HOSTING_TYPE: &str = "Content";
pub const ACCESS_TYPE: &str = "Cable/DSL/ISP";
pub const INFRASTRUCTURE_TYPES: [&str; 3] = ["NSP", "Network Services", "Enterprise"];
pub const MAX_USERS_PER_ADDRESS: f64 = 0.05;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct HostingStats {
    pub by_type: usize,
    pub by_population: usize,
}

pub fn ipv4_addresses(selected: &[SelectedRoute]) -> HashMap<Asn, u64> {
    let mut routes: Vec<(IpNet, Asn)> = selected
        .iter()
        .filter(|r| matches!(r.prefix, IpNet::V4(_)))
        .map(|r| (r.prefix, r.asn))
        .collect();
    routes.sort_by_key(|(prefix, _)| (prefix.network(), prefix.prefix_len()));
    let mut addresses: HashMap<Asn, i64> = HashMap::new();
    let mut enclosing: Vec<(IpNet, Asn)> = Vec::new();
    for (prefix, asn) in routes {
        while enclosing
            .last()
            .is_some_and(|(outer, _)| !outer.contains(&prefix))
        {
            enclosing.pop();
        }
        let size = 1i64 << (32 - prefix.prefix_len());
        if let Some((_, outer)) = enclosing.last() {
            *addresses.entry(*outer).or_default() -= size;
        }
        *addresses.entry(asn).or_default() += size;
        enclosing.push((prefix, asn));
    }
    addresses
        .into_iter()
        .map(|(asn, count)| (asn, count.max(0) as u64))
        .collect()
}

pub fn hosting_ases(
    types: &HashMap<Asn, Vec<String>>,
    users: &HashMap<Asn, u64>,
    addresses: &HashMap<Asn, u64>,
) -> (HashSet<Asn>, HostingStats) {
    let mut stats = HostingStats::default();
    let mut hosting = HashSet::new();
    for (&asn, types) in types {
        let has = |t: &str| types.iter().any(|x| x == t);
        if has(ACCESS_TYPE) {
            continue;
        }
        if has(HOSTING_TYPE) {
            stats.by_type += 1;
            hosting.insert(asn);
        } else if INFRASTRUCTURE_TYPES.iter().any(|t| has(t)) {
            let users = users.get(&asn).copied().unwrap_or(0) as f64;
            let addresses = addresses.get(&asn).copied().unwrap_or(0) as f64;
            if users < MAX_USERS_PER_ADDRESS * addresses {
                stats.by_population += 1;
                hosting.insert(asn);
            }
        }
    }
    (hosting, stats)
}

pub fn anonymous_ip_db(
    selected: &[SelectedRoute],
    hosting: &HashSet<Asn>,
    build_epoch: u64,
) -> (Writer, usize) {
    let mut db = writer(
        "GeoIP2-Anonymous-IP",
        "Prefixes originated by hosting providers, from PeeringDB and APNIC user estimates",
        build_epoch,
    );
    let record = Value::map([
        ("is_anonymous", Value::Bool(true)),
        ("is_hosting_provider", Value::Bool(true)),
    ]);
    let mut prefixes = 0;
    for route in selected {
        if hosting.contains(&route.asn) {
            prefixes += 1;
            db.insert(route.prefix, &record);
        } else {
            db.remove(route.prefix);
        }
    }
    clear_special_ranges(&mut db);
    (db, prefixes)
}

#[cfg(test)]
mod tests {
    use maxminddb::path;
    use src_rpki::Validity;

    use super::*;
    use crate::testing::{get, read};

    fn route(prefix: &str, asn: Asn) -> SelectedRoute {
        SelectedRoute {
            prefix: prefix.parse().unwrap(),
            asn,
            validity: Validity::NotFound,
        }
    }

    fn types(entries: &[(Asn, &[&str])]) -> HashMap<Asn, Vec<String>> {
        entries
            .iter()
            .map(|(asn, types)| (*asn, types.iter().map(|t| t.to_string()).collect()))
            .collect()
    }

    #[test]
    fn counts_the_addresses_each_as_ends_up_with() {
        let selected = [
            route("11.0.0.0/16", 64500),
            route("11.0.1.0/24", 64501),
            route("11.0.1.128/25", 64500),
            route("12.0.0.0/24", 64501),
            route("2001:db8::/32", 64502),
        ];
        assert_eq!(
            ipv4_addresses(&selected),
            HashMap::from([(64500, 65536 - 256 + 128), (64501, 128 + 256)])
        );
    }

    #[test]
    fn classifies_ases_by_type_then_by_users_per_address() {
        let types = types(&[
            (64500, &["Content"]),
            (64501, &["Content", "Cable/DSL/ISP"]),
            (64502, &["NSP"]),
            (64503, &["NSP"]),
            (64504, &["Enterprise", "Network Services"]),
            (64505, &["Educational/Research"]),
            (64506, &[]),
        ]);
        let users = HashMap::from([(64500, 5_000_000), (64502, 400), (64503, 600)]);
        let addresses = HashMap::from([
            (64500, 1024),
            (64502, 10_000),
            (64503, 10_000),
            (64505, 10_000),
            (64506, 10_000),
        ]);
        let (hosting, stats) = hosting_ases(&types, &users, &addresses);
        assert_eq!(hosting, HashSet::from([64500, 64502]));
        assert_eq!(
            stats,
            HostingStats {
                by_type: 1,
                by_population: 1,
            }
        );
    }

    #[test]
    fn flags_hosting_prefixes_but_not_more_specifics_of_other_ases() {
        let selected = [
            route("11.0.0.0/16", 64500),
            route("12.0.0.0/16", 64501),
            route("11.0.1.0/24", 64501),
            route("12.0.1.0/24", 64500),
        ];
        let (db, prefixes) = anonymous_ip_db(&selected, &HashSet::from([64500]), 0);
        assert_eq!(prefixes, 2);
        let reader = read(db);
        let hosting = |ip: &str| get::<bool>(&reader, ip, &path!["is_hosting_provider"]);
        assert_eq!(hosting("11.0.0.1"), Some(true));
        assert_eq!(hosting("11.0.1.1"), None);
        assert_eq!(hosting("12.0.0.1"), None);
        assert_eq!(hosting("12.0.1.1"), Some(true));
        assert_eq!(
            get::<bool>(&reader, "12.0.1.1", &path!["is_anonymous"]),
            Some(true)
        );
        assert_eq!(reader.metadata().database_type, "GeoIP2-Anonymous-IP");
    }
}
