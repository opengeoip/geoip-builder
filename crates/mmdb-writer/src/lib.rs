use std::collections::{HashMap, VecDeque};
use std::io::{self, Write};

use ipnet::IpNet;

const METADATA_MARKER: &[u8] = b"\xAB\xCD\xEFMaxMind.com";
const RECORD_SIZE: u16 = 32;
const IPV4_ALIASES: [(u128, u8); 2] = [(0xffff << 32, 96), (0x2002 << 112, 16)];

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    String(String),
    Double(f64),
    Bytes(Vec<u8>),
    U16(u16),
    U32(u32),
    Map(Vec<(String, Value)>),
    I32(i32),
    U64(u64),
    Array(Vec<Value>),
    Bool(bool),
}

impl Value {
    pub fn map<K: Into<String>>(entries: impl IntoIterator<Item = (K, Value)>) -> Value {
        Value::Map(entries.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }

    pub fn string(s: impl Into<String>) -> Value {
        Value::String(s.into())
    }

    fn encode(&self, out: &mut Vec<u8>) {
        match self {
            Value::String(s) => encode_str(out, s),
            Value::Double(d) => {
                header(out, 3, 8);
                out.extend_from_slice(&d.to_be_bytes());
            }
            Value::Bytes(b) => {
                header(out, 4, b.len());
                out.extend_from_slice(b);
            }
            Value::U16(v) => encode_uint(out, 5, u64::from(*v)),
            Value::U32(v) => encode_uint(out, 6, u64::from(*v)),
            Value::Map(entries) => {
                header(out, 7, entries.len());
                for (key, value) in entries {
                    encode_str(out, key);
                    value.encode(out);
                }
            }
            Value::I32(v) => {
                header(out, 8, 4);
                out.extend_from_slice(&v.to_be_bytes());
            }
            Value::U64(v) => encode_uint(out, 9, *v),
            Value::Array(items) => {
                header(out, 11, items.len());
                for item in items {
                    item.encode(out);
                }
            }
            Value::Bool(b) => header(out, 14, usize::from(*b)),
        }
    }
}

fn encode_str(out: &mut Vec<u8>, s: &str) {
    header(out, 2, s.len());
    out.extend_from_slice(s.as_bytes());
}

fn encode_uint(out: &mut Vec<u8>, kind: u8, value: u64) {
    let bytes = value.to_be_bytes();
    let skip = (value.leading_zeros() / 8) as usize;
    header(out, kind, 8 - skip);
    out.extend_from_slice(&bytes[skip..]);
}

fn header(out: &mut Vec<u8>, kind: u8, size: usize) {
    let (size_bits, extra): (u8, Vec<u8>) = match size {
        0..29 => (size as u8, Vec::new()),
        29..285 => (29, vec![(size - 29) as u8]),
        285..65821 => (30, ((size - 285) as u16).to_be_bytes().to_vec()),
        _ => (31, ((size - 65821) as u32).to_be_bytes()[1..].to_vec()),
    };
    if kind <= 7 {
        out.push(kind << 5 | size_bits);
    } else {
        out.push(size_bits);
        out.push(kind - 7);
    }
    out.extend_from_slice(&extra);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Record {
    Empty,
    Node(u32),
    Data(u32),
}

pub struct Writer {
    nodes: Vec<[Record; 2]>,
    data: Vec<u8>,
    offsets: HashMap<Vec<u8>, u32>,
    database_type: String,
    description: Vec<(String, String)>,
    languages: Vec<String>,
    build_epoch: u64,
}

impl Writer {
    pub fn new(database_type: impl Into<String>) -> Self {
        Self {
            nodes: vec![[Record::Empty; 2]],
            data: Vec::new(),
            offsets: HashMap::new(),
            database_type: database_type.into(),
            description: Vec::new(),
            languages: Vec::new(),
            build_epoch: 0,
        }
    }

    pub fn description(mut self, language: impl Into<String>, text: impl Into<String>) -> Self {
        self.description.push((language.into(), text.into()));
        self
    }

    pub fn language(mut self, language: impl Into<String>) -> Self {
        self.languages.push(language.into());
        self
    }

    pub fn build_epoch(mut self, epoch: u64) -> Self {
        self.build_epoch = epoch;
        self
    }

    pub fn insert(&mut self, network: IpNet, value: &Value) {
        let (bits, length) = key(network);
        let offset = self.data_offset(value);
        self.set(bits, length, Record::Data(offset));
    }

    pub fn remove(&mut self, network: IpNet) {
        let (bits, length) = key(network);
        self.set(bits, length, Record::Empty);
    }

    fn data_offset(&mut self, value: &Value) -> u32 {
        let mut encoded = Vec::new();
        value.encode(&mut encoded);
        if let Some(&offset) = self.offsets.get(&encoded) {
            return offset;
        }
        let offset = self.data.len() as u32;
        self.data.extend_from_slice(&encoded);
        self.offsets.insert(encoded, offset);
        offset
    }

    fn set(&mut self, bits: u128, length: u8, record: Record) {
        if length == 0 {
            self.nodes[0] = [record; 2];
            return;
        }
        let mut node = 0usize;
        for depth in 0..length - 1 {
            let bit = bit_at(bits, depth);
            node = match self.nodes[node][bit] {
                Record::Node(next) => next as usize,
                leaf => {
                    let next = self.nodes.len();
                    self.nodes.push([leaf; 2]);
                    self.nodes[node][bit] = Record::Node(next as u32);
                    next
                }
            };
        }
        self.nodes[node][bit_at(bits, length - 1)] = record;
    }

    fn record_at(&self, bits: u128, length: u8) -> Record {
        let mut record = Record::Node(0);
        for depth in 0..length {
            match record {
                Record::Node(node) => record = self.nodes[node as usize][bit_at(bits, depth)],
                leaf => return leaf,
            }
        }
        record
    }

    fn collapse(&mut self, record: Record, memo: &mut HashMap<u32, Record>) -> Record {
        let Record::Node(node) = record else {
            return record;
        };
        if let Some(&done) = memo.get(&node) {
            return done;
        }
        let [left, right] = self.nodes[node as usize];
        let left = self.collapse(left, memo);
        let right = self.collapse(right, memo);
        self.nodes[node as usize] = [left, right];
        let result = if left == right && !matches!(left, Record::Node(_)) {
            left
        } else {
            record
        };
        memo.insert(node, result);
        result
    }

    pub fn write_to<W: Write>(mut self, mut out: W) -> io::Result<()> {
        let mut memo = HashMap::new();
        let [left, right] = self.nodes[0];
        let left = self.collapse(left, &mut memo);
        let right = self.collapse(right, &mut memo);
        self.nodes[0] = [left, right];

        let ipv4_root = self.record_at(0, 96);
        if ipv4_root != Record::Empty {
            for (bits, length) in IPV4_ALIASES {
                self.set(bits, length, ipv4_root);
            }
        }

        let mut order = Vec::new();
        let mut ids = vec![u32::MAX; self.nodes.len()];
        let mut queue = VecDeque::from([0u32]);
        ids[0] = 0;
        while let Some(node) = queue.pop_front() {
            order.push(node);
            for record in self.nodes[node as usize] {
                if let Record::Node(child) = record
                    && ids[child as usize] == u32::MAX
                {
                    ids[child as usize] = order.len() as u32 + queue.len() as u32;
                    queue.push_back(child);
                }
            }
        }

        let node_count = order.len() as u32;
        let encode = |record: Record| -> u32 {
            match record {
                Record::Empty => node_count,
                Record::Node(node) => ids[node as usize],
                Record::Data(offset) => node_count + 16 + offset,
            }
        };
        let mut tree = Vec::with_capacity(order.len() * 8);
        for node in &order {
            for record in self.nodes[*node as usize] {
                tree.extend_from_slice(&encode(record).to_be_bytes());
            }
        }
        out.write_all(&tree)?;
        out.write_all(&[0u8; 16])?;
        out.write_all(&self.data)?;
        out.write_all(METADATA_MARKER)?;

        let metadata = Value::map([
            ("binary_format_major_version", Value::U16(2)),
            ("binary_format_minor_version", Value::U16(0)),
            ("build_epoch", Value::U64(self.build_epoch)),
            ("database_type", Value::String(self.database_type)),
            (
                "description",
                Value::Map(
                    self.description
                        .into_iter()
                        .map(|(k, v)| (k, Value::String(v)))
                        .collect(),
                ),
            ),
            ("ip_version", Value::U16(6)),
            (
                "languages",
                Value::Array(self.languages.into_iter().map(Value::String).collect()),
            ),
            ("node_count", Value::U32(node_count)),
            ("record_size", Value::U16(RECORD_SIZE)),
        ]);
        let mut encoded = Vec::new();
        metadata.encode(&mut encoded);
        out.write_all(&encoded)?;
        out.flush()
    }
}

fn key(network: IpNet) -> (u128, u8) {
    match network.trunc() {
        IpNet::V4(net) => (u128::from(u32::from(net.addr())), net.prefix_len() + 96),
        IpNet::V6(net) => (u128::from(net.addr()), net.prefix_len()),
    }
}

fn bit_at(bits: u128, depth: u8) -> usize {
    ((bits >> (127 - depth)) & 1) as usize
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use maxminddb::Reader;
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize, Debug, PartialEq)]
    struct Country<'a> {
        iso_code: &'a str,
    }

    #[derive(Deserialize, Debug, PartialEq)]
    struct Record<'a> {
        #[serde(borrow)]
        country: Country<'a>,
        rank: Option<u32>,
    }

    fn country(code: &str) -> Value {
        Value::map([("country", Value::map([("iso_code", Value::string(code))]))])
    }

    fn build(entries: &[(&str, Value)]) -> Vec<u8> {
        let mut writer = Writer::new("Test")
            .description("en", "test")
            .language("en")
            .build_epoch(1);
        for (network, value) in entries {
            writer.insert(network.parse().unwrap(), value);
        }
        let mut out = Vec::new();
        writer.write_to(&mut out).unwrap();
        out
    }

    fn lookup(reader: &Reader<Vec<u8>>, ip: &str) -> Option<String> {
        let ip: IpAddr = ip.parse().unwrap();
        reader
            .lookup(ip)
            .unwrap()
            .decode::<Record>()
            .unwrap()
            .map(|r| r.country.iso_code.to_string())
    }

    #[test]
    fn round_trips_through_the_reference_reader() {
        let bytes = build(&[
            ("1.0.0.0/8", country("AU")),
            ("1.2.3.0/24", country("FR")),
            ("2001:db8::/32", country("DE")),
            ("2001:db8:1::/48", country("AU")),
        ]);
        let reader = Reader::from_source(bytes).unwrap();
        reader.verify().unwrap();
        assert_eq!(reader.metadata().database_type, "Test");
        assert_eq!(lookup(&reader, "1.1.1.1").as_deref(), Some("AU"));
        assert_eq!(lookup(&reader, "1.2.3.4").as_deref(), Some("FR"));
        assert_eq!(lookup(&reader, "1.2.4.4").as_deref(), Some("AU"));
        assert_eq!(lookup(&reader, "2.0.0.1"), None);
        assert_eq!(lookup(&reader, "2001:db8::1").as_deref(), Some("DE"));
        assert_eq!(lookup(&reader, "2001:db8:1::1").as_deref(), Some("AU"));
        assert_eq!(lookup(&reader, "::ffff:1.2.3.4").as_deref(), Some("FR"));
        assert_eq!(lookup(&reader, "2002:102:304::1").as_deref(), Some("FR"));
        let ip: IpAddr = "1.2.3.4".parse().unwrap();
        assert_eq!(
            reader.lookup(ip).unwrap().network().unwrap().to_string(),
            "1.2.3.0/24"
        );
    }

    #[test]
    fn collapses_adjacent_identical_networks() {
        let bytes = build(&[
            ("10.0.0.0/9", country("NL")),
            ("10.128.0.0/9", country("NL")),
        ]);
        let reader = Reader::from_source(bytes).unwrap();
        reader.verify().unwrap();
        let ip: IpAddr = "10.200.0.1".parse().unwrap();
        assert_eq!(
            reader.lookup(ip).unwrap().network().unwrap().to_string(),
            "10.0.0.0/8"
        );
    }

    #[test]
    fn encodes_every_type() {
        let value = Value::map([
            (
                "country",
                Value::map([("iso_code", Value::string("x".repeat(70000)))]),
            ),
            ("rank", Value::U32(70000)),
            ("big", Value::U64(u64::MAX)),
            ("zero", Value::U16(0)),
            ("neg", Value::I32(-5)),
            ("ok", Value::Bool(true)),
            ("pi", Value::Double(3.5)),
            ("raw", Value::Bytes(vec![1, 2, 3])),
            ("list", Value::Array(vec![Value::string("a"); 300])),
        ]);
        let bytes = build(&[("192.0.2.0/24", value)]);
        let reader = Reader::from_source(bytes).unwrap();
        reader.verify().unwrap();
        let ip: IpAddr = "192.0.2.1".parse().unwrap();
        let record: Record = reader.lookup(ip).unwrap().decode().unwrap().unwrap();
        assert_eq!(record.country.iso_code.len(), 70000);
        assert_eq!(record.rank, Some(70000));
    }
}
