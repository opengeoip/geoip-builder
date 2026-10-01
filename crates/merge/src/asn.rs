use std::collections::HashMap;

use ipnet::IpNet;
use mmdb_writer::{Value, Writer};
use model::{AsName, Asn, Route};
use src_rpki::{Validator, Validity};

use crate::{clear_special_ranges, writer};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RpkiPolicy {
    #[default]
    RejectInvalid,
    ValidOnly,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct AsnStats {
    pub routes: usize,
    pub valid: usize,
    pub invalid: usize,
    pub not_found: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectedRoute {
    pub prefix: IpNet,
    pub asn: Asn,
    pub validity: Validity,
}

pub fn select_origins(
    routes: &[Route],
    validator: &Validator,
    policy: RpkiPolicy,
) -> (Vec<SelectedRoute>, AsnStats) {
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
                selected.push(SelectedRoute {
                    prefix: route.prefix,
                    asn: origin.asn,
                    validity: Validity::Valid,
                });
            }
            (None, Validity::Invalid) => stats.invalid += 1,
            (None, _) => {
                stats.not_found += 1;
                if policy == RpkiPolicy::RejectInvalid
                    && let Some(origin) = route.origins.first()
                {
                    selected.push(SelectedRoute {
                        prefix: route.prefix,
                        asn: origin.asn,
                        validity: Validity::NotFound,
                    });
                }
            }
        }
    }
    selected.sort_by_key(|route| (route.prefix.prefix_len(), route.prefix));
    (selected, stats)
}

pub fn asn_db(
    selected: &[SelectedRoute],
    names: &HashMap<Asn, AsName>,
    policy: RpkiPolicy,
    build_epoch: u64,
) -> (Writer, usize) {
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
    let mut unnamed = 0;
    for route in selected {
        let rpki = match route.validity {
            Validity::Valid => "valid",
            Validity::Invalid => "invalid",
            Validity::NotFound => "not-found",
        };
        let mut entries = vec![
            ("autonomous_system_number", Value::U32(route.asn)),
            ("rpki", Value::string(rpki)),
        ];
        match names.get(&route.asn) {
            Some(name) => entries.push((
                "autonomous_system_organization",
                Value::string(name.organization.as_deref().unwrap_or(&name.handle)),
            )),
            None => unnamed += 1,
        }
        db.insert(route.prefix, &Value::map(entries));
    }
    clear_special_ranges(&mut db);
    (db, unnamed)
}

#[cfg(test)]
mod tests {
    use maxminddb::{Reader, path};
    use model::{Origin, Vrp};

    use super::*;
    use crate::testing::{get, read};

    fn route(prefix: &str, origins: &[(Asn, u32)]) -> Route {
        Route {
            prefix: prefix.parse().unwrap(),
            origins: origins
                .iter()
                .map(|&(asn, peers)| Origin { asn, peers })
                .collect(),
        }
    }

    fn fixture(policy: RpkiPolicy) -> (Reader<Vec<u8>>, AsnStats, usize) {
        let routes = vec![
            route("11.0.0.0/16", &[(64500, 10)]),
            route("11.0.1.0/24", &[(64666, 50), (64500, 3)]),
            route("11.0.2.0/24", &[(64666, 50)]),
            route("193.0.2.0/24", &[(64501, 50)]),
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
        let (selected, stats) = select_origins(&routes, &validator, policy);
        let (db, unnamed) = asn_db(&selected, &names, policy, 0);
        (read(db), stats, unnamed)
    }

    const STATS: AsnStats = AsnStats {
        routes: 4,
        valid: 2,
        invalid: 1,
        not_found: 1,
    };

    #[test]
    fn valid_only_keeps_rpki_valid_origins() {
        let (reader, stats, unnamed) = fixture(RpkiPolicy::ValidOnly);
        assert_eq!((stats, unnamed), (STATS, 0));
        assert_eq!(
            get::<u32>(&reader, "11.0.1.1", &path!["autonomous_system_number"]),
            Some(64500)
        );
        assert_eq!(
            get::<u32>(&reader, "11.0.2.1", &path!["autonomous_system_number"]),
            Some(64500)
        );
        assert_eq!(
            get::<u32>(&reader, "193.0.2.1", &path!["autonomous_system_number"]),
            None
        );
        assert_eq!(
            get::<String>(
                &reader,
                "11.0.2.1",
                &path!["autonomous_system_organization"]
            )
            .as_deref(),
            Some("Example Ltd")
        );
    }

    #[test]
    fn reject_invalid_also_keeps_routes_without_roa() {
        let (reader, stats, unnamed) = fixture(RpkiPolicy::RejectInvalid);
        assert_eq!((stats, unnamed), (STATS, 1));
        assert_eq!(
            get::<u32>(&reader, "11.0.2.1", &path!["autonomous_system_number"]),
            Some(64500)
        );
        assert_eq!(
            get::<String>(&reader, "11.0.2.1", &path!["rpki"]).as_deref(),
            Some("valid")
        );
        assert_eq!(
            get::<u32>(&reader, "193.0.2.1", &path!["autonomous_system_number"]),
            Some(64501)
        );
        assert_eq!(
            get::<String>(&reader, "193.0.2.1", &path!["rpki"]).as_deref(),
            Some("not-found")
        );
    }
}
