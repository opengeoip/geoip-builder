mod asn;
mod geofeed;
mod listed;
mod location;

pub use asn::{AsnStats, RpkiPolicy, SelectedRoute, asn_db, select_origins};
pub use geofeed::{GeofeedStats, authorize_geofeeds};
pub use listed::{ListedFeed, ListedGeofeed, authorize_listed_geofeeds};
pub use location::{LocationStats, location_dbs};

use mmdb_writer::Writer;

pub const LANGUAGES: [&str; 1] = ["en"];

fn writer(database_type: &str, description: &str, build_epoch: u64) -> Writer {
    LANGUAGES
        .iter()
        .fold(Writer::new(database_type), |w, l| w.language(*l))
        .description("en", description)
        .build_epoch(build_epoch)
}

fn clear_special_ranges(db: &mut Writer) {
    for range in model::special::ranges() {
        db.remove(range.network);
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use std::net::IpAddr;

    use maxminddb::{PathElement, Reader};
    use mmdb_writer::Writer;
    use serde::Deserialize;

    pub fn read(writer: Writer) -> Reader<Vec<u8>> {
        let mut out = Vec::new();
        writer.write_to(&mut out).unwrap();
        let reader = Reader::from_source(out).unwrap();
        reader.verify().unwrap();
        reader
    }

    pub fn get<'a, T: Deserialize<'a>>(
        reader: &'a Reader<Vec<u8>>,
        ip: &str,
        path: &[PathElement<'_>],
    ) -> Option<T> {
        let ip: IpAddr = ip.parse().unwrap();
        reader.lookup(ip).unwrap().decode_path(path).unwrap()
    }
}
