use std::collections::HashMap;
use std::io::Read;

use anyhow::Result;
use model::Asn;
use serde::Deserialize;

pub const URL: &str =
    "https://www.peeringdb.com/api/net?fields=asn,name,website,info_type,info_types";

#[derive(Deserialize)]
struct Response {
    data: Vec<Network>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Network {
    pub asn: Asn,
    pub name: String,
    #[serde(default)]
    pub website: Option<String>,
    #[serde(default)]
    pub info_type: Option<String>,
    #[serde(default)]
    pub info_types: Option<Vec<String>>,
}

impl Network {
    pub fn types(&self) -> Vec<&str> {
        match &self.info_types {
            Some(types) if !types.is_empty() => types.iter().map(String::as_str).collect(),
            _ => self
                .info_type
                .as_deref()
                .filter(|t| !t.is_empty())
                .into_iter()
                .collect(),
        }
    }
}

pub fn parse<R: Read>(reader: R) -> Result<HashMap<Asn, Network>> {
    let response: Response = serde_json::from_reader(reader)?;
    Ok(response.data.into_iter().map(|n| (n.asn, n)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_networks_and_their_types() {
        let input = r#"{"data":[
            {"asn":16276,"name":"OVHcloud","website":"https://www.ovhcloud.com","info_type":"Content","info_types":["Content"]},
            {"asn":64500,"name":"Mixed","website":"","info_type":"","info_types":["Cable/DSL/ISP","NSP"]},
            {"asn":64501,"name":"Old","info_type":"NSP"},
            {"asn":64502,"name":"Untyped","info_type":"","info_types":[]}
        ]}"#;
        let networks = parse(input.as_bytes()).unwrap();
        assert_eq!(networks.len(), 4);
        assert_eq!(networks[&16276].types(), ["Content"]);
        assert_eq!(networks[&64500].types(), ["Cable/DSL/ISP", "NSP"]);
        assert_eq!(networks[&64501].types(), ["NSP"]);
        assert!(networks[&64502].types().is_empty());
    }
}
