pub mod audit;
pub mod candidates;
pub mod compare;
pub mod coverage;
pub mod evaluate;
pub mod hosting;
pub mod lookup;

#[cfg(test)]
pub(crate) mod testing {
    use maxminddb::Reader;
    use mmdb_writer::{Value, Writer};

    pub fn database(entries: &[(&str, Value)]) -> Reader<Vec<u8>> {
        let mut writer = Writer::new("Test");
        for (network, value) in entries {
            writer.insert(network.parse().unwrap(), value);
        }
        let mut bytes = Vec::new();
        writer.write_to(&mut bytes).unwrap();
        Reader::from_source(bytes).unwrap()
    }

    pub fn country(code: &str) -> Value {
        Value::map([("country", Value::map([("iso_code", Value::string(code))]))])
    }

    pub fn asn(number: u32) -> Value {
        Value::map([("autonomous_system_number", Value::U32(number))])
    }
}
