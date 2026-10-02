use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use ipnet::IpNet;
use serde::{Deserialize, Serialize};

pub const MANUAL: &str = "manual";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Row {
    pub url: String,
    pub network: Option<IpNet>,
    pub source: String,
}

impl Row {
    pub fn is_manual(&self) -> bool {
        self.source == MANUAL
    }
}

pub fn read(path: &Path) -> Result<Vec<Row>> {
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
    let rows: BTreeSet<Row> = rows.into_iter().collect();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn row(url: &str, network: Option<&str>, source: &str) -> Row {
        Row {
            url: url.into(),
            network: network.map(|n| n.parse().unwrap()),
            source: source.into(),
        }
    }

    #[test]
    fn keeps_manual_rows_across_discoveries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("geofeeds.csv");
        fs::write(
            &path,
            "url,network,source\n\
             # added by hand\n\
             https://cloud.example/feed.csv,,manual\n\
             https://isp.example/feed.csv, 192.0.2.0/24 ,manual\n\
             https://old.example/feed.csv,198.51.100.0/24,ripencc\n",
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
                row("https://cloud.example/feed.csv", None, "manual"),
                row(
                    "https://isp.example/feed.csv",
                    Some("192.0.2.0/24"),
                    "manual"
                ),
                row(
                    "https://new.example/feed.csv",
                    Some("203.0.113.0/24"),
                    "arin"
                ),
            ]
        );
    }
}
