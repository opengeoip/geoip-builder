pub mod prefix_map;
pub mod special;

pub use prefix_map::PrefixMap;

use std::fmt;
use std::str::FromStr;

use ipnet::IpNet;

pub type Asn = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Registry {
    Afrinic,
    Apnic,
    Arin,
    Lacnic,
    RipeNcc,
}

impl Registry {
    pub const ALL: [Registry; 5] = [
        Registry::Afrinic,
        Registry::Apnic,
        Registry::Arin,
        Registry::Lacnic,
        Registry::RipeNcc,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Registry::Afrinic => "afrinic",
            Registry::Apnic => "apnic",
            Registry::Arin => "arin",
            Registry::Lacnic => "lacnic",
            Registry::RipeNcc => "ripencc",
        }
    }
}

impl fmt::Display for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct UnknownRegistry(pub String);

impl fmt::Display for UnknownRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown registry {:?}", self.0)
    }
}

impl std::error::Error for UnknownRegistry {}

impl FromStr for Registry {
    type Err = UnknownRegistry;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "afrinic" => Ok(Registry::Afrinic),
            "apnic" => Ok(Registry::Apnic),
            "arin" => Ok(Registry::Arin),
            "lacnic" => Ok(Registry::Lacnic),
            "ripencc" | "ripe" => Ok(Registry::RipeNcc),
            other => Err(UnknownRegistry(other.to_string())),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Delegation {
    pub network: IpNet,
    pub country: String,
    pub registry: Registry,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AsName {
    pub handle: String,
    pub organization: Option<String>,
    pub country: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Vrp {
    pub prefix: IpNet,
    pub max_length: u8,
    pub asn: Asn,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Origin {
    pub asn: Asn,
    pub peers: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    pub prefix: IpNet,
    pub origins: Vec<Origin>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeofeedRef {
    pub network: IpNet,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    pub network: IpNet,
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub postal: Option<String>,
}
