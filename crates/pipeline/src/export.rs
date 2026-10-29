use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};
use flate2::Compression;
use flate2::write::GzEncoder;
use ipnetwork::IpNetwork;
use maxminddb::{Reader, WithinOptions};
use model::continent::continent;
use model::hash::fnv1a;
use serde::Deserialize;

use crate::Log;

const LOCALE: &str = "en";
const FIRST_PLACE_ID: u32 = 1 << 24;
const EUROPEAN_UNION: [&str; 27] = [
    "AT", "BE", "BG", "CY", "CZ", "DE", "DK", "EE", "ES", "FI", "FR", "GR", "HR", "HU", "IE", "IT",
    "LT", "LU", "LV", "MT", "NL", "PL", "PT", "RO", "SE", "SI", "SK",
];

#[derive(Deserialize)]
struct Iso<'a> {
    iso_code: &'a str,
}

#[derive(Deserialize)]
struct Names<'a> {
    #[serde(borrow)]
    names: BTreeMap<&'a str, &'a str>,
}

#[derive(Deserialize)]
struct Postal<'a> {
    code: &'a str,
}

#[derive(Deserialize)]
struct LocationRecord<'a> {
    #[serde(borrow)]
    country: Option<Iso<'a>>,
    #[serde(borrow)]
    registered_country: Option<Iso<'a>>,
    #[serde(borrow, default)]
    subdivisions: Vec<Iso<'a>>,
    #[serde(borrow)]
    city: Option<Names<'a>>,
    #[serde(borrow)]
    postal: Option<Postal<'a>>,
}

#[derive(Deserialize)]
struct AsnRecord<'a> {
    autonomous_system_number: u32,
    autonomous_system_organization: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Place {
    country: String,
    subdivision: Option<String>,
    city: Option<String>,
}

impl Place {
    fn country(code: &str) -> Place {
        Place {
            country: code.to_string(),
            subdivision: None,
            city: None,
        }
    }
}

struct Block {
    network: IpNetwork,
    place: Option<Place>,
    registered: Option<String>,
    postal: Option<String>,
}

const FAMILIES: [(&str, &str); 2] = [("ipv4", "0.0.0.0/0"), ("ipv6", "::/0")];

fn networks<S: AsRef<[u8]>, F>(reader: &Reader<S>, scope: &str, mut row: F) -> Result<()>
where
    F: FnMut(IpNetwork, &maxminddb::LookupResult<'_, S>) -> Result<()>,
{
    let scope: IpNetwork = scope.parse()?;
    for item in reader.within(scope, WithinOptions::default())? {
        let item = item?;
        let network = item.network()?;
        if network.is_ipv4() == scope.is_ipv4() {
            row(network, &item)?;
        }
    }
    Ok(())
}

fn location_blocks<S: AsRef<[u8]>>(reader: &Reader<S>, scope: &str) -> Result<Vec<Block>> {
    let mut blocks = Vec::new();
    networks(reader, scope, |network, item| {
        let Some(record) = item.decode::<LocationRecord>()? else {
            return Ok(());
        };
        let place = record.country.map(|country| Place {
            country: country.iso_code.to_string(),
            subdivision: record.subdivisions.first().map(|s| s.iso_code.to_string()),
            city: record
                .city
                .and_then(|c| c.names.get(LOCALE).map(|n| n.to_string())),
        });
        blocks.push(Block {
            network,
            place,
            registered: record.registered_country.map(|c| c.iso_code.to_string()),
            postal: record.postal.map(|p| p.code.to_string()),
        });
        Ok(())
    })?;
    Ok(blocks)
}

fn country_id(code: &str) -> u32 {
    let [a, b] = code.as_bytes() else {
        return 0;
    };
    u32::from(*a) << 8 | u32::from(*b)
}

fn place_ids(places: &BTreeSet<Place>) -> BTreeMap<Place, u32> {
    let mut ids = BTreeMap::new();
    let mut taken = BTreeSet::new();
    for place in places {
        let id = if place.subdivision.is_none() && place.city.is_none() {
            country_id(&place.country)
        } else {
            let key = format!(
                "{}\0{}\0{}",
                place.country,
                place.subdivision.as_deref().unwrap_or_default(),
                place.city.as_deref().unwrap_or_default()
            );
            (0..)
                .map(|seed| {
                    FIRST_PLACE_ID
                        + (fnv1a(seed, key.as_bytes())
                            % u64::from(i32::MAX as u32 - FIRST_PLACE_ID))
                            as u32
                })
                .find(|id| !taken.contains(id))
                .unwrap_or_default()
        };
        taken.insert(id);
        ids.insert(place.clone(), id);
    }
    ids
}

fn write_blocks<W: Write>(
    out: W,
    blocks: &[Block],
    ids: &BTreeMap<Place, u32>,
    city: bool,
) -> Result<()> {
    let mut out = csv::Writer::from_writer(out);
    let mut header = vec![
        "network",
        "geoname_id",
        "registered_country_geoname_id",
        "represented_country_geoname_id",
        "is_anonymous_proxy",
        "is_satellite_provider",
    ];
    if city {
        header.extend(["postal_code", "latitude", "longitude", "accuracy_radius"]);
    }
    header.push("is_anycast");
    out.write_record(&header)?;
    let id = |place: Option<&Place>| {
        place
            .and_then(|p| ids.get(p))
            .map(u32::to_string)
            .unwrap_or_default()
    };
    for block in blocks {
        let place = block.place.as_ref().map(|p| {
            if city {
                p.clone()
            } else {
                Place::country(&p.country)
            }
        });
        let registered = block.registered.as_deref().map(Place::country);
        let mut row = vec![
            block.network.to_string(),
            id(place.as_ref()),
            id(registered.as_ref()),
            String::new(),
        ];
        row.extend(["0".to_string(), "0".to_string()]);
        if city {
            row.extend([
                block.postal.clone().unwrap_or_default(),
                String::new(),
                String::new(),
                String::new(),
            ]);
        }
        row.push(String::new());
        out.write_record(&row)?;
    }
    out.flush()?;
    Ok(())
}

fn write_locations<W: Write>(out: W, ids: &BTreeMap<Place, u32>, city: bool) -> Result<()> {
    let mut out = csv::Writer::from_writer(out);
    let mut header = vec![
        "geoname_id",
        "locale_code",
        "continent_code",
        "continent_name",
        "country_iso_code",
        "country_name",
    ];
    if city {
        header.extend([
            "subdivision_1_iso_code",
            "subdivision_1_name",
            "subdivision_2_iso_code",
            "subdivision_2_name",
            "city_name",
            "metro_code",
            "time_zone",
        ]);
    }
    header.push("is_in_european_union");
    out.write_record(&header)?;
    let mut rows: Vec<(&u32, &Place)> = ids.iter().map(|(place, id)| (id, place)).collect();
    rows.sort();
    for (id, place) in rows {
        let continent = continent(&place.country);
        let mut row = vec![
            id.to_string(),
            LOCALE.to_string(),
            continent.map(|c| c.code()).unwrap_or_default().to_string(),
            continent.map(|c| c.name()).unwrap_or_default().to_string(),
            place.country.clone(),
            String::new(),
        ];
        if city {
            row.extend([
                place.subdivision.clone().unwrap_or_default(),
                String::new(),
                String::new(),
                String::new(),
                place.city.clone().unwrap_or_default(),
                String::new(),
                String::new(),
            ]);
        }
        row.push(u8::from(EUROPEAN_UNION.contains(&place.country.as_str())).to_string());
        out.write_record(&row)?;
    }
    out.flush()?;
    Ok(())
}

fn create(
    dir: &Path,
    name: &str,
    log: &mut Log<'_>,
    write: impl FnOnce(&mut GzEncoder<BufWriter<File>>) -> Result<()>,
) -> Result<()> {
    let path = dir.join(name);
    let partial = path.with_extension("gz.part");
    let file = File::create(&partial).with_context(|| format!("creating {}", partial.display()))?;
    let mut out = GzEncoder::new(BufWriter::new(file), Compression::default());
    write(&mut out)?;
    out.finish()?.flush()?;
    fs::rename(&partial, &path)?;
    log(format!(
        "wrote {} ({} bytes)",
        path.display(),
        fs::metadata(&path)?.len()
    ));
    Ok(())
}

pub fn locations<S: AsRef<[u8]>>(
    reader: &Reader<S>,
    dir: &Path,
    name: &str,
    city: bool,
    log: &mut Log<'_>,
) -> Result<()> {
    let mut families = Vec::new();
    let mut places = BTreeSet::new();
    for (label, scope) in FAMILIES {
        let blocks = location_blocks(reader, scope)?;
        for block in &blocks {
            if let Some(place) = &block.place {
                places.insert(Place::country(&place.country));
                if city {
                    places.insert(place.clone());
                }
            }
            if let Some(registered) = &block.registered {
                places.insert(Place::country(registered));
            }
        }
        families.push((label, blocks));
    }
    let ids = place_ids(&places);
    for (label, blocks) in &families {
        create(dir, &format!("{name}-blocks-{label}.csv.gz"), log, |file| {
            write_blocks(file, blocks, &ids, city)
        })?;
    }
    create(
        dir,
        &format!("{name}-locations-{LOCALE}.csv.gz"),
        log,
        |file| write_locations(file, &ids, city),
    )
}

pub fn asn<S: AsRef<[u8]>>(
    reader: &Reader<S>,
    dir: &Path,
    name: &str,
    log: &mut Log<'_>,
) -> Result<()> {
    for (label, scope) in FAMILIES {
        create(dir, &format!("{name}-blocks-{label}.csv.gz"), log, |file| {
            let mut out = csv::Writer::from_writer(file);
            out.write_record([
                "network",
                "autonomous_system_number",
                "autonomous_system_organization",
            ])?;
            networks(reader, scope, |network, item| {
                if let Some(record) = item.decode::<AsnRecord>()? {
                    out.write_record([
                        network.to_string(),
                        record.autonomous_system_number.to_string(),
                        record
                            .autonomous_system_organization
                            .unwrap_or_default()
                            .to_string(),
                    ])?;
                }
                Ok(())
            })?;
            out.flush()?;
            Ok(())
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use mmdb_writer::{Value, Writer};

    use super::*;
    use crate::io::in_memory;

    fn iso(code: &str) -> Value {
        Value::map([("iso_code", Value::string(code))])
    }

    fn read(dir: &Path, name: &str) -> Vec<String> {
        let mut text = String::new();
        flate2::read::GzDecoder::new(File::open(dir.join(name)).unwrap())
            .read_to_string(&mut text)
            .unwrap();
        text.lines().map(String::from).collect()
    }

    #[test]
    fn writes_geolite2_city_csv() {
        let mut writer = Writer::new("GeoLite2-City");
        writer.insert(
            "90.0.0.0/9".parse().unwrap(),
            &Value::map([("country", iso("FR")), ("registered_country", iso("FR"))]),
        );
        writer.insert(
            "2a01:cb00::/32".parse().unwrap(),
            &Value::map([
                ("country", iso("DE")),
                ("registered_country", iso("FR")),
                ("subdivisions", Value::Array(vec![iso("HE")])),
                (
                    "city",
                    Value::map([("names", Value::map([("en", Value::string("Frankfurt"))]))]),
                ),
                ("postal", Value::map([("code", Value::string("60313"))])),
            ]),
        );
        let reader = in_memory(writer).unwrap();
        let dir = tempfile::tempdir().unwrap();
        locations(&reader, dir.path(), "city", true, &mut |_| {}).unwrap();

        let france = country_id("FR");
        let germany = country_id("DE");
        assert_eq!(
            read(dir.path(), "city-blocks-ipv4.csv.gz")[1..],
            [format!("90.0.0.0/9,{france},{france},,0,0,,,,,")]
        );
        let ipv6 = read(dir.path(), "city-blocks-ipv6.csv.gz");
        assert_eq!(ipv6.len(), 2);
        let fields: Vec<&str> = ipv6[1].split(',').collect();
        assert_eq!(fields[0], "2a01:cb00::/32");
        assert_eq!(fields[2], france.to_string());
        assert_eq!(fields[6], "60313");

        let places = read(dir.path(), "city-locations-en.csv.gz");
        assert_eq!(
            places[1..],
            [
                format!("{germany},en,EU,Europe,DE,,,,,,,,,1"),
                format!("{france},en,EU,Europe,FR,,,,,,,,,1"),
                format!("{},en,EU,Europe,DE,,HE,,,,Frankfurt,,,1", fields[1]),
            ]
        );
    }

    #[test]
    fn place_ids_are_stable_and_distinct() {
        let place = |city: &str| Place {
            country: "FR".into(),
            subdivision: None,
            city: Some(city.into()),
        };
        let places: BTreeSet<Place> = [Place::country("FR"), place("Paris"), place("Lyon")].into();
        let ids = place_ids(&places);
        assert_eq!(ids[&Place::country("FR")], country_id("FR"));
        assert!(ids[&place("Paris")] >= FIRST_PLACE_ID);
        assert_ne!(ids[&place("Paris")], ids[&place("Lyon")]);
        assert_eq!(ids, place_ids(&places));
    }
}
