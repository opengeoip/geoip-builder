use mmdb_writer::{Value, Writer};
use model::{Delegation, Location, PrefixMap, Registry};

use crate::{clear_special_ranges, writer};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct LocationStats {
    pub delegations: usize,
    pub geofeed_entries: usize,
}

fn iso(code: &str) -> Value {
    Value::map([("iso_code", Value::string(code))])
}

fn base(country: &str, registered: Option<(&str, Registry)>) -> Vec<(&'static str, Value)> {
    let mut entries = vec![("country", iso(country))];
    if let Some((registered, registry)) = registered {
        entries.push(("registered_country", iso(registered)));
        entries.push(("registry", Value::string(registry.as_str())));
    }
    entries
}

pub fn location_dbs(
    mut delegations: Vec<Delegation>,
    mut locations: Vec<Location>,
    build_epoch: u64,
) -> (Writer, Writer, LocationStats) {
    delegations.sort_by_key(|d| (d.network.prefix_len(), d.network));
    locations.sort_by_key(|l| l.network.prefix_len());
    let mut country_db = writer(
        "GeoLite2-Country",
        "Country from RFC 8805 geofeeds, else of the registrant from the RIR statistics",
        build_epoch,
    );
    let mut city_db = writer(
        "GeoLite2-City",
        "Location from RFC 8805 geofeeds, else country of the registrant from the RIR statistics",
        build_epoch,
    );

    let mut registered: PrefixMap<(&str, Registry)> = PrefixMap::default();
    for delegation in &delegations {
        registered.insert(
            delegation.network,
            (&delegation.country, delegation.registry),
        );
        let value = Value::map(base(
            &delegation.country,
            Some((&delegation.country, delegation.registry)),
        ));
        country_db.insert(delegation.network, &value);
        city_db.insert(delegation.network, &value);
    }

    for location in &locations {
        let Some(country) = location.country.as_deref() else {
            continue;
        };
        let owner = registered
            .longest_match(location.network)
            .map(|(_, owner)| *owner);
        let mut entries = base(country, owner);
        country_db.insert(location.network, &Value::map(entries.clone()));
        if let Some(region) = &location.region {
            entries.push(("subdivisions", Value::Array(vec![iso(region)])));
        }
        if let Some(city) = &location.city {
            entries.push((
                "city",
                Value::map([("names", Value::map([("en", Value::string(city))]))]),
            ));
        }
        if let Some(postal) = &location.postal {
            entries.push(("postal", Value::map([("code", Value::string(postal))])));
        }
        city_db.insert(location.network, &Value::map(entries));
    }

    clear_special_ranges(&mut country_db);
    clear_special_ranges(&mut city_db);
    let stats = LocationStats {
        delegations: delegations.len(),
        geofeed_entries: locations.len(),
    };
    (country_db, city_db, stats)
}

#[cfg(test)]
mod tests {
    use maxminddb::path;

    use super::*;
    use crate::testing::{get, read};

    fn delegation(network: &str, country: &str, registry: Registry) -> Delegation {
        Delegation {
            network: network.parse().unwrap(),
            country: country.into(),
            registry,
        }
    }

    #[test]
    fn more_specific_delegations_win() {
        let delegations = vec![
            delegation("2a00:1450:1::/48", "FR", Registry::RipeNcc),
            delegation("2a00:1450::/32", "DE", Registry::RipeNcc),
        ];
        let (db, _, _) = location_dbs(delegations, Vec::new(), 0);
        let reader = read(db);
        let country = |ip| get::<String>(&reader, ip, &path!["country", "iso_code"]);
        assert_eq!(country("2a00:1450:1::1").as_deref(), Some("FR"));
        assert_eq!(country("2a00:1450:2::1").as_deref(), Some("DE"));
    }

    #[test]
    fn special_purpose_ranges_never_get_data() {
        let delegations = vec![
            delegation("192.0.0.0/8", "US", Registry::Arin),
            delegation("2001:db8::/32", "DE", Registry::RipeNcc),
        ];
        let (db, _, _) = location_dbs(delegations, Vec::new(), 0);
        let reader = read(db);
        let country = |ip| get::<String>(&reader, ip, &path!["country", "iso_code"]);
        assert_eq!(country("192.1.2.3").as_deref(), Some("US"));
        assert_eq!(country("192.168.1.1"), None);
        assert_eq!(country("192.0.2.1"), None);
        assert_eq!(country("2001:db8::1"), None);
        assert_eq!(country("2002:c001:203::1").as_deref(), Some("US"));
        assert_eq!(country("2002:c0a8:101::1"), None);
    }

    #[test]
    fn geofeeds_override_the_registrant_country() {
        let delegations = vec![delegation("3.0.0.0/8", "US", Registry::Arin)];
        let locations = vec![Location {
            network: "3.64.0.0/12".parse().unwrap(),
            country: Some("DE".into()),
            region: Some("HE".into()),
            city: Some("Frankfurt am Main".into()),
            postal: None,
        }];
        let (country_db, city_db, stats) = location_dbs(delegations, locations, 0);
        assert_eq!(
            stats,
            LocationStats {
                delegations: 1,
                geofeed_entries: 1
            }
        );
        let country_db = read(country_db);
        let city_db = read(city_db);
        let get_str =
            |reader, ip, path: &[maxminddb::PathElement<'_>]| get::<String>(reader, ip, path);
        assert_eq!(
            get_str(&country_db, "3.64.1.1", &path!["country", "iso_code"]).as_deref(),
            Some("DE")
        );
        assert_eq!(
            get_str(
                &country_db,
                "3.64.1.1",
                &path!["registered_country", "iso_code"]
            )
            .as_deref(),
            Some("US")
        );
        assert_eq!(
            get_str(&country_db, "3.64.1.1", &path!["city", "names", "en"]),
            None
        );
        assert_eq!(
            get_str(&city_db, "3.64.1.1", &path!["city", "names", "en"]).as_deref(),
            Some("Frankfurt am Main")
        );
        assert_eq!(
            get_str(&city_db, "3.64.1.1", &path!["subdivisions", 0, "iso_code"]).as_deref(),
            Some("HE")
        );
        assert_eq!(
            get_str(&city_db, "3.1.1.1", &path!["country", "iso_code"]).as_deref(),
            Some("US")
        );
    }
}
