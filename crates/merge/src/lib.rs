use std::collections::HashMap;

use mmdb_writer::{Value, Writer};
use model::{AsName, Asn, Delegation, Route};
use src_rpki::{Validator, Validity};

pub const LANGUAGES: [&str; 1] = ["en"];

fn clear_special_ranges(db: &mut Writer) {
    for range in model::special::ranges() {
        db.remove(range.network);
    }
}

fn writer(database_type: &str, description: &str, build_epoch: u64) -> Writer {
    LANGUAGES
        .iter()
        .fold(Writer::new(database_type), |w, l| w.language(*l))
        .description("en", description)
        .build_epoch(build_epoch)
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct CountryStats {
    pub networks: usize,
}

pub fn country_db(mut delegations: Vec<Delegation>, build_epoch: u64) -> (Writer, CountryStats) {
    delegations.sort_by_key(|d| (d.network.prefix_len(), d.network));
    let mut db = writer(
        "GeoLite2-Country",
        "Country of the registrant, from the RIR delegated-extended statistics",
        build_epoch,
    );
    for delegation in &delegations {
        let country = Value::map([("iso_code", Value::string(&delegation.country))]);
        let value = Value::map([
            ("country", country.clone()),
            ("registered_country", country),
            ("registry", Value::string(delegation.registry.as_str())),
        ]);
        db.insert(delegation.network, &value);
    }
    clear_special_ranges(&mut db);
    (
        db,
        CountryStats {
            networks: delegations.len(),
        },
    )
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct AsnStats {
    pub routes: usize,
    pub valid: usize,
    pub invalid: usize,
    pub not_found: usize,
    pub unnamed: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RpkiPolicy {
    #[default]
    RejectInvalid,
    ValidOnly,
}

pub fn asn_db(
    routes: &[Route],
    validator: &Validator,
    names: &HashMap<Asn, AsName>,
    policy: RpkiPolicy,
    build_epoch: u64,
) -> (Writer, AsnStats) {
    let mut stats = AsnStats {
        routes: routes.len(),
        ..Default::default()
    };
    let mut selected = Vec::new();
    for route in routes {
        let mut worst = Validity::NotFound;
        let valid = route.origins.iter().find(|origin| {
            match validator.validate(route.prefix, origin.asn) {
                Validity::Valid => true,
                Validity::Invalid => {
                    worst = Validity::Invalid;
                    false
                }
                Validity::NotFound => false,
            }
        });
        match (valid, worst) {
            (Some(origin), _) => {
                stats.valid += 1;
                selected.push((route.prefix, origin.asn, Validity::Valid));
            }
            (None, Validity::Invalid) => stats.invalid += 1,
            (None, _) => {
                stats.not_found += 1;
                if policy == RpkiPolicy::RejectInvalid
                    && let Some(origin) = route.origins.first()
                {
                    selected.push((route.prefix, origin.asn, Validity::NotFound));
                }
            }
        }
    }
    selected.sort_by_key(|(prefix, _, _)| (prefix.prefix_len(), *prefix));

    let mut db = writer(
        "GeoLite2-ASN",
        match policy {
            RpkiPolicy::RejectInvalid => {
                "Origin AS of announced prefixes, RPKI-invalid routes excluded"
            }
            RpkiPolicy::ValidOnly => "Origin AS of announced prefixes, RPKI-valid routes only",
        },
        build_epoch,
    );
    for (prefix, asn, validity) in selected {
        let rpki = match validity {
            Validity::Valid => "valid",
            Validity::Invalid => "invalid",
            Validity::NotFound => "not-found",
        };
        let mut entries = vec![
            ("autonomous_system_number", Value::U32(asn)),
            ("rpki", Value::string(rpki)),
        ];
        match names.get(&asn) {
            Some(name) => entries.push((
                "autonomous_system_organization",
                Value::string(name.organization.as_deref().unwrap_or(&name.handle)),
            )),
            None => stats.unnamed += 1,
        }
        db.insert(prefix, &Value::map(entries));
    }
    clear_special_ranges(&mut db);
    (db, stats)
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use maxminddb::Reader;
    use model::{Origin, Registry, Vrp};

    use super::*;

    fn read(writer: Writer) -> Reader<Vec<u8>> {
        let mut out = Vec::new();
        writer.write_to(&mut out).unwrap();
        Reader::from_source(out).unwrap()
    }

    fn asn_of(reader: &Reader<Vec<u8>>, ip: &str) -> Option<u32> {
        let ip: IpAddr = ip.parse().unwrap();
        reader
            .lookup(ip)
            .unwrap()
            .decode_path(&maxminddb::path!["autonomous_system_number"])
            .unwrap()
    }

    fn fixture(policy: RpkiPolicy) -> (Reader<Vec<u8>>, AsnStats) {
        let routes = vec![
            Route {
                prefix: "11.0.0.0/16".parse().unwrap(),
                origins: vec![Origin {
                    asn: 64500,
                    peers: 10,
                }],
            },
            Route {
                prefix: "11.0.1.0/24".parse().unwrap(),
                origins: vec![
                    Origin {
                        asn: 64666,
                        peers: 50,
                    },
                    Origin {
                        asn: 64500,
                        peers: 3,
                    },
                ],
            },
            Route {
                prefix: "11.0.2.0/24".parse().unwrap(),
                origins: vec![Origin {
                    asn: 64666,
                    peers: 50,
                }],
            },
            Route {
                prefix: "193.0.2.0/24".parse().unwrap(),
                origins: vec![Origin {
                    asn: 64501,
                    peers: 50,
                }],
            },
        ];
        let validator = Validator::new(&[Vrp {
            prefix: "11.0.0.0/16".parse().unwrap(),
            max_length: 24,
            asn: 64500,
        }]);
        let names = HashMap::from([(
            64500,
            AsName {
                handle: "EXAMPLE".into(),
                organization: Some("Example Ltd".into()),
                country: None,
            },
        )]);
        let (db, stats) = asn_db(&routes, &validator, &names, policy, 0);
        let reader = read(db);
        reader.verify().unwrap();
        (reader, stats)
    }

    fn rpki_of(reader: &Reader<Vec<u8>>, ip: &str) -> Option<String> {
        let ip: IpAddr = ip.parse().unwrap();
        reader
            .lookup(ip)
            .unwrap()
            .decode_path(&maxminddb::path!["rpki"])
            .unwrap()
    }

    const STATS: AsnStats = AsnStats {
        routes: 4,
        valid: 2,
        invalid: 1,
        not_found: 1,
        unnamed: 0,
    };

    #[test]
    fn valid_only_keeps_rpki_valid_origins() {
        let (reader, stats) = fixture(RpkiPolicy::ValidOnly);
        assert_eq!(stats, STATS);
        assert_eq!(asn_of(&reader, "11.0.1.1"), Some(64500));
        assert_eq!(asn_of(&reader, "11.0.2.1"), Some(64500));
        assert_eq!(asn_of(&reader, "193.0.2.1"), None);
        let ip: IpAddr = "11.0.2.1".parse().unwrap();
        let org: Option<String> = reader
            .lookup(ip)
            .unwrap()
            .decode_path(&maxminddb::path!["autonomous_system_organization"])
            .unwrap();
        assert_eq!(org.as_deref(), Some("Example Ltd"));
    }

    #[test]
    fn reject_invalid_also_keeps_routes_without_roa() {
        let (reader, stats) = fixture(RpkiPolicy::RejectInvalid);
        assert_eq!(
            stats,
            AsnStats {
                unnamed: 1,
                ..STATS
            }
        );
        assert_eq!(asn_of(&reader, "11.0.1.1"), Some(64500));
        assert_eq!(asn_of(&reader, "11.0.2.1"), Some(64500));
        assert_eq!(rpki_of(&reader, "11.0.2.1").as_deref(), Some("valid"));
        assert_eq!(asn_of(&reader, "193.0.2.1"), Some(64501));
        assert_eq!(rpki_of(&reader, "193.0.2.1").as_deref(), Some("not-found"));
    }

    #[test]
    fn more_specific_delegations_win() {
        let delegations = vec![
            Delegation {
                network: "2a00:1450:1::/48".parse().unwrap(),
                country: "FR".into(),
                registry: Registry::RipeNcc,
            },
            Delegation {
                network: "2a00:1450::/32".parse().unwrap(),
                country: "DE".into(),
                registry: Registry::RipeNcc,
            },
        ];
        let (db, _) = country_db(delegations, 0);
        let reader = read(db);
        let country = |ip: &str| -> Option<String> {
            let ip: IpAddr = ip.parse().unwrap();
            reader
                .lookup(ip)
                .unwrap()
                .decode_path(&maxminddb::path!["country", "iso_code"])
                .unwrap()
        };
        assert_eq!(country("2a00:1450:1::1").as_deref(), Some("FR"));
        assert_eq!(country("2a00:1450:2::1").as_deref(), Some("DE"));
    }

    #[test]
    fn special_purpose_ranges_never_get_data() {
        let delegations = vec![
            Delegation {
                network: "192.0.0.0/8".parse().unwrap(),
                country: "US".into(),
                registry: Registry::Arin,
            },
            Delegation {
                network: "2001:db8::/32".parse().unwrap(),
                country: "DE".into(),
                registry: Registry::RipeNcc,
            },
        ];
        let (db, _) = country_db(delegations, 0);
        let reader = read(db);
        reader.verify().unwrap();
        let country = |ip: &str| -> Option<String> {
            let ip: IpAddr = ip.parse().unwrap();
            reader
                .lookup(ip)
                .unwrap()
                .decode_path(&maxminddb::path!["country", "iso_code"])
                .unwrap()
        };
        assert_eq!(country("192.1.2.3").as_deref(), Some("US"));
        assert_eq!(country("192.168.1.1"), None);
        assert_eq!(country("192.0.2.1"), None);
        assert_eq!(country("2001:db8::1"), None);
        assert_eq!(country("2002:c001:203::1").as_deref(), Some("US"));
        assert_eq!(country("2002:c0a8:101::1"), None);
    }
}
