# geoip-builder

[![CI](https://github.com/opengeoip/geoip-builder/actions/workflows/ci.yml/badge.svg)](https://github.com/opengeoip/geoip-builder/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/opengeoip/geoip-builder)](https://github.com/opengeoip/geoip-builder/releases)
[![License](https://img.shields.io/github/license/opengeoip/geoip-builder)](LICENSE)

geoip-builder builds IP geolocation, ASN and hosting provider databases from public data only, as `.mmdb` files that any MaxMind DB library can read. They follow the GeoLite2 Country, City and ASN schemas and the GeoIP2 Anonymous IP schema, so they work as drop-in replacements.

- **Country and city** come from the internet registries, refined by the geofeeds operators publish ([RFC 8805](https://www.rfc-editor.org/rfc/rfc8805)).
- **ASN** comes from BGP routing tables, with RPKI-invalid routes left out.
- **Hosting providers** come from the network type operators declare in PeeringDB and from APNIC's estimates of users per AS.
- **No WHOIS, no rate-limited API, no commercial data**: the databases can be rebuilt every day.

## Install

Download a static Linux binary from the [releases](https://github.com/opengeoip/geoip-builder/releases), use the container image, or build from source:

```sh
sudo apt install cargo
git clone https://github.com/opengeoip/geoip-builder.git
cd geoip-builder
cargo build --release
```

```sh
docker run --rm -v "$PWD/data:/work/data" -v "$PWD/out:/work/out" ghcr.io/opengeoip/geoip-builder run
```

## Quick start

```sh
geoip-builder run
geoip-builder lookup out/country.mmdb 1.1.1.1
geoip-builder lookup out/anonymous-ip.mmdb 51.15.0.1
```

The first run downloads about 1 GB and takes about 10 minutes. The databases land in `out/`.

## Documentation

- [Usage](docs/usage.md): commands and options
- [Sources](docs/sources.md): where the data comes from
- [How it works](docs/databases.md): how each database is built
- [Accuracy](docs/evaluation.md): how it compares to GeoLite2
- [Development](docs/development.md): code, CI and releases

## License

GPL-3.0-or-later, see [LICENSE](LICENSE). The generated databases derive from third-party data with their own terms of use (registries, RIPE RIS and Atlas, RPKI, PeeringDB, APNIC Labs, operators' geofeeds): check them before redistributing the files.
