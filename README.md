# geoip-builder

geoip-builder builds IP geolocation and ASN databases in the [MaxMind DB](https://maxmind.github.io/MaxMind-DB/) format, compatible with the GeoLite2 Country, City and ASN schemas, from public bulk data only: RIR statistics and databases, BGP routing tables, RPKI and the geofeeds operators publish ([RFC 8805](https://www.rfc-editor.org/rfc/rfc8805), [RFC 9632](https://www.rfc-editor.org/rfc/rfc9632)). It uses no WHOIS, no rate-limited API and no commercial database, and can be rebuilt as often as needed.

- `country.mmdb`, `city.mmdb` and `asn.mmdb`, readable by any MaxMind DB library;
- country from the registries, refined by sub-allocations and by operators' geofeeds;
- origin AS from BGP, with RPKI-invalid routes excluded;
- a versioned catalog of the geofeeds in use, and tools to measure accuracy and find missing geofeeds.

## Installation

The build needs a Rust toolchain; on Debian or Ubuntu:

```sh
sudo apt install cargo
git clone <repository> geoip-builder
cd geoip-builder
cargo build --release
```

The binary is `target/release/geoip-builder`.

## Quick start

```sh
geoip-builder run --data-dir data --out-dir out
geoip-builder lookup out/country.mmdb 1.1.1.1
geoip-builder lookup out/asn.mmdb 2a01:cb00::1
```

The first run downloads about 1 GB of public data and takes about 10 minutes.

## Documentation

- [Usage](docs/usage.md): commands and options
- [Sources](docs/sources.md): the data used and where it comes from
- [Databases](docs/databases.md): how each database is built
- [Evaluation](docs/evaluation.md): accuracy measurements
- [Development](docs/development.md): code layout and checks

## License

geoip-builder, including its geofeed catalog, is free software under the [GNU General Public License](LICENSE), version 3 or any later version.

The databases it builds are derived from third-party data, each under its own terms of use (RIR statistics and databases, RIPE RIS, RPKI repositories, PeeringDB, RIPE Atlas, operators' geofeeds): check them before redistributing the generated files.
