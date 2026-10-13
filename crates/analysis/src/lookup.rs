use std::net::IpAddr;

use anyhow::Result;
use maxminddb::{Reader, path};

pub fn country(reader: &Reader<Vec<u8>>, address: IpAddr) -> Result<Option<String>> {
    Ok(reader
        .lookup(address)?
        .decode_path::<String>(&path!["country", "iso_code"])?)
}

pub fn asn(reader: &Reader<Vec<u8>>, address: IpAddr) -> Result<Option<u32>> {
    Ok(reader
        .lookup(address)?
        .decode_path::<u32>(&path!["autonomous_system_number"])?)
}

pub fn hosting_provider(reader: &Reader<Vec<u8>>, address: IpAddr) -> Result<bool> {
    Ok(reader
        .lookup(address)?
        .decode_path::<bool>(&path!["is_hosting_provider"])?
        .unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{asn as asn_value, country as country_value, database};

    #[test]
    fn reads_country_and_asn() {
        let countries = database(&[("192.0.2.0/24", country_value("FR"))]);
        let asns = database(&[("192.0.2.0/24", asn_value(64500))]);
        let inside: IpAddr = "192.0.2.1".parse().unwrap();
        let outside: IpAddr = "198.51.100.1".parse().unwrap();
        assert_eq!(country(&countries, inside).unwrap().as_deref(), Some("FR"));
        assert_eq!(country(&countries, outside).unwrap(), None);
        assert_eq!(asn(&asns, inside).unwrap(), Some(64500));
    }
}
