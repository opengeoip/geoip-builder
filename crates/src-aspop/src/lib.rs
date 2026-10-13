use std::collections::HashMap;
use std::io::Read;

use anyhow::{Result, bail};
use model::Asn;
use serde::Deserialize;

pub const URL: &str = "https://stats.labs.apnic.net/cgi-bin/aspopjson";

#[derive(Deserialize)]
struct Row {
    asn: String,
    users: u64,
}

pub fn parse<R: Read>(reader: R) -> Result<HashMap<Asn, u64>> {
    let rows: Vec<Row> = serde_json::from_reader(reader)?;
    let mut users = HashMap::new();
    for row in rows {
        let Some(asn) = row.asn.strip_prefix("AS").and_then(|a| a.parse().ok()) else {
            bail!("unexpected AS {:?}", row.asn);
        };
        *users.entry(asn).or_default() += row.users;
    }
    Ok(users)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_users_over_countries() {
        let input = r#"[
            {"rank": 1, "asn": "AS3215", "descr": "AS3215 - Orange S.A.", "cc": "FR", "users": 19735544, "country-percent": 36, "internet-percent": 0, "samples": 31851120},
            {"rank": 40, "asn": "AS3215", "descr": "AS3215 - Orange S.A.", "cc": "RE", "users": 1000, "country-percent": 1, "internet-percent": 0, "samples": 2000},
            {"rank": 3, "asn": "AS16276", "descr": "OVH", "cc": "FR", "users": 0, "country-percent": 0, "internet-percent": 0, "samples": 4}
        ]"#;
        let users = parse(input.as_bytes()).unwrap();
        assert_eq!(users, HashMap::from([(3215, 19736544), (16276, 0)]));
        assert!(parse(r#"[{"asn": "x", "users": 1}]"#.as_bytes()).is_err());
    }
}
