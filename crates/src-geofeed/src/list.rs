use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use ipnet::IpNet;
use model::Asn;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const MANUAL: &str = "manual";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Row {
    pub url: String,
    pub network: Option<IpNet>,
    pub source: String,
    #[serde(
        default,
        serialize_with = "serialize_asns",
        deserialize_with = "deserialize_asns"
    )]
    pub asn: Vec<Asn>,
}

fn serialize_asns<S: Serializer>(asns: &[Asn], serializer: S) -> Result<S::Ok, S::Error> {
    let text: Vec<String> = asns.iter().map(Asn::to_string).collect();
    serializer.serialize_str(&text.join(" "))
}

fn deserialize_asns<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Asn>, D::Error> {
    let text = String::deserialize(deserializer)?;
    let mut asns = text
        .split_whitespace()
        .map(|asn| asn.parse().map_err(serde::de::Error::custom))
        .collect::<Result<Vec<Asn>, _>>()?;
    asns.sort_unstable();
    asns.dedup();
    Ok(asns)
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ManualRow {
    pub url: String,
    #[serde(
        default,
        serialize_with = "serialize_asns",
        deserialize_with = "deserialize_asns"
    )]
    pub asn: Vec<Asn>,
}

impl From<ManualRow> for Row {
    fn from(row: ManualRow) -> Self {
        Row {
            url: row.url,
            network: None,
            source: MANUAL.to_string(),
            asn: row.asn,
        }
    }
}

impl Row {
    pub fn is_manual(&self) -> bool {
        self.source == MANUAL
    }
}

pub fn read(path: &Path) -> Result<Vec<Row>> {
    read_csv(path)
}

pub fn read_manual(path: &Path) -> Result<Vec<Row>> {
    let rows: Vec<ManualRow> = read_csv(path)?;
    Ok(rows.into_iter().map(Row::from).collect())
}

fn read_csv<T: DeserializeOwned>(path: &Path) -> Result<Vec<T>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut reader = csv::ReaderBuilder::new()
        .comment(Some(b'#'))
        .trim(csv::Trim::All)
        .from_path(path)
        .with_context(|| format!("opening {}", path.display()))?;
    reader
        .deserialize()
        .enumerate()
        .map(|(index, row)| row.with_context(|| format!("{} line {}", path.display(), index + 2)))
        .collect()
}

pub fn write(path: &Path, rows: impl IntoIterator<Item = Row>) -> Result<usize> {
    write_csv(path, rows)
}

pub fn write_manual(path: &Path, rows: impl IntoIterator<Item = Row>) -> Result<usize> {
    write_csv(
        path,
        rows.into_iter().map(|row| ManualRow {
            url: row.url,
            asn: row.asn,
        }),
    )
}

fn write_csv<T: Serialize + Ord>(path: &Path, rows: impl IntoIterator<Item = T>) -> Result<usize> {
    let rows: BTreeSet<T> = rows.into_iter().collect();
    let partial = path.with_extension("csv.part");
    {
        let mut writer = csv::Writer::from_path(&partial)?;
        for row in &rows {
            writer.serialize(row)?;
        }
        writer.flush()?;
    }
    fs::rename(&partial, path)?;
    Ok(rows.len())
}

pub fn merge_discovered(path: &Path, discovered: impl IntoIterator<Item = Row>) -> Result<usize> {
    let manual: Vec<Row> = read(path)?.into_iter().filter(Row::is_manual).collect();
    write(path, manual.into_iter().chain(discovered))
}

pub fn problems(rows: &[Row]) -> Vec<String> {
    rows.iter()
        .filter_map(|row| {
            let scheme = row
                .url
                .split_once("://")
                .map(|(scheme, _)| scheme.to_ascii_lowercase());
            match scheme.as_deref() {
                Some("http" | "https") if !row.url.contains(char::is_whitespace) => None,
                _ => Some(format!("invalid URL {:?}", row.url)),
            }
            .or_else(|| {
                row.source
                    .is_empty()
                    .then(|| format!("{}: empty source", row.url))
            })
            .or_else(|| {
                (row.is_manual() && row.network.is_some())
                    .then(|| format!("{}: network set on a manual geofeed", row.url))
            })
            .or_else(|| {
                (row.network.is_none() && row.asn.is_empty())
                    .then(|| format!("{}: unanchored geofeed without asn", row.url))
            })
            .or_else(|| {
                (row.network.is_some() && !row.asn.is_empty())
                    .then(|| format!("{}: asn set on an anchored geofeed", row.url))
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(url: &str, network: Option<&str>, source: &str) -> Row {
        Row {
            url: url.into(),
            network: network.map(|n| n.parse().unwrap()),
            source: source.into(),
            asn: Vec::new(),
        }
    }

    fn listed(url: &str, asn: &[Asn]) -> Row {
        Row {
            asn: asn.to_vec(),
            ..row(url, None, MANUAL)
        }
    }

    #[test]
    fn reports_invalid_rows() {
        let rows = [
            listed("https://a.example/feed.csv", &[64500]),
            listed("ftp://b.example/feed.csv", &[64500]),
            Row {
                source: String::new(),
                ..listed("https://c.example/feed.csv", &[64500])
            },
            row("https://d.example/feed.csv", None, "manual"),
            Row {
                asn: vec![64500],
                ..row(
                    "https://e.example/feed.csv",
                    Some("192.0.2.0/24"),
                    "ripencc",
                )
            },
            row("https://f.example/feed.csv", Some("192.0.2.0/24"), "manual"),
        ];
        assert_eq!(
            problems(&rows),
            [
                "invalid URL \"ftp://b.example/feed.csv\"",
                "https://c.example/feed.csv: empty source",
                "https://d.example/feed.csv: unanchored geofeed without asn",
                "https://e.example/feed.csv: asn set on an anchored geofeed",
                "https://f.example/feed.csv: network set on a manual geofeed",
            ]
        );
    }

    #[test]
    fn keeps_manual_rows_across_discoveries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("geofeeds.csv");
        fs::write(
            &path,
            "url,network,source,asn\n\
             # added by hand\n\
             https://cloud.example/feed.csv,,manual,64501 64500 64501\n\
             https://isp.example/feed.csv,,manual, 64502 \n\
             https://old.example/feed.csv,198.51.100.0/24,ripencc,\n",
        )
        .unwrap();
        let count = merge_discovered(
            &path,
            [row(
                "https://new.example/feed.csv",
                Some("203.0.113.0/24"),
                "arin",
            )],
        )
        .unwrap();
        assert_eq!(count, 3);
        assert_eq!(
            read(&path).unwrap(),
            [
                listed("https://cloud.example/feed.csv", &[64500, 64501]),
                listed("https://isp.example/feed.csv", &[64502]),
                row(
                    "https://new.example/feed.csv",
                    Some("203.0.113.0/24"),
                    "arin"
                ),
            ]
        );
    }

    #[test]
    fn reads_and_writes_manual_lists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("manual.csv");
        fs::write(
            &path,
            "url,asn\n\
             # added by hand\n\
             https://cloud.example/feed.csv,64501 64500\n",
        )
        .unwrap();
        let rows = read_manual(&path).unwrap();
        assert_eq!(
            rows,
            [listed("https://cloud.example/feed.csv", &[64500, 64501])]
        );
        write_manual(&path, rows).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "url,asn\nhttps://cloud.example/feed.csv,64500 64501\n"
        );
    }

    #[test]
    fn reads_catalogs_without_the_asn_column() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("geofeeds.csv");
        fs::write(
            &path,
            "url,network,source\nhttps://cloud.example/feed.csv,,manual\n",
        )
        .unwrap();
        assert_eq!(
            read(&path).unwrap(),
            [row("https://cloud.example/feed.csv", None, "manual")]
        );
    }
}
