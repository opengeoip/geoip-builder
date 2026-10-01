# geoip-builder

Builds MaxMind DB (`.mmdb`) files, compatible with the GeoLite2 Country and ASN schemas, from public bulk data only: no WHOIS, no RDAP, no rate-limited API.

## Usage

```sh
cargo build --release
target/release/geoip-builder run --data-dir data --out-dir out
target/release/geoip-builder lookup out/asn.mmdb 1.1.1.1 2a01:cb00::1
target/release/geoip-builder compare --kind country out/country.mmdb GeoLite2-Country.mmdb
```

- `fetch` downloads every source into `--data-dir`, with conditional requests (`If-None-Match`, `If-Modified-Since`), so running it often costs almost nothing when nothing changed.
- `build` reads only `--data-dir` and writes `country.mmdb` and `asn.mmdb` into `--out-dir`. A build never touches the network, so it can be replayed on a saved data directory.
- `run` is `fetch` followed by `build`.
- `--rpki-valid-only` restricts the ASN database to RPKI-valid routes (see below).
- `--collector` (repeatable, default `rrc00`) selects the RIPE RIS collectors whose RIB dumps are used. `--vrps-url` points to another VRP export, such as a local Routinator.
- `lookup` prints the network and record matching each address.
- `compare` measures how far a database is from a reference one, weighted by IPv4 addresses and IPv6 /48 networks, with the top disagreements.

A full run takes about 3 minutes, almost all of it downloading the RIB dump, and peaks at about 450 MB of memory.

## Sources

| Source | File | Used for |
|---|---|---|
| [RIR delegated-extended statistics](https://www.nro.net/about/rirs/statistics/) of AFRINIC, APNIC, ARIN, LACNIC and RIPE NCC | `delegated-<rir>` | country of every allocated or assigned IPv4 and IPv6 block |
| [RIPE NCC AS names](https://ftp.ripe.net/ripe/asnames/asn.txt), compiled from the five RIRs | `asn.txt` | AS organization names |
| [rpki-client VRP export](https://console.rpki-client.org/) | `vrps.json` | route origin validation |
| [RIPE RIS](https://ris.ripe.net/) RIB dumps (MRT) | `ris-<collector>.bview.gz` | origin AS of every announced prefix |

## Country database

Each record holds `country.iso_code`, `registered_country.iso_code` (both the same value) and `registry`.

The country is the one the RIR registered for the holder of the block, not a measured location. It is right for most access networks and often wrong for cloud and CDN ranges registered in one country and used worldwide. Blocks with a status other than `allocated` or `assigned`, and the `ZZ` code, are skipped. IPv4 ranges whose size is not a power of two are split into the minimal set of CIDR blocks. Delegations are inserted from the least to the most specific, so a sub-allocation overrides its parent.

## ASN database

Each record holds `autonomous_system_number`, `autonomous_system_organization` and `rpki`, the [RFC 6811](https://www.rfc-editor.org/rfc/rfc6811) state of the kept route (`valid` or `not-found`).

RPKI-invalid routes are never kept. For every prefix seen in the RIB dumps:

1. the origin of each path is the last AS of its `AS_PATH` (merged with `AS4_PATH`); paths ending in an `AS_SET` of more than one AS are ignored;
2. origins are ranked by the number of distinct peers announcing them;
3. if at least one VRP covers the prefix, the best-ranked origin whose (prefix, origin) pair is `Valid` is kept, and the prefix is dropped when none is;
4. if no VRP covers the prefix (`NotFound`), the best-ranked origin is kept, unless `--rpki-valid-only` is set.

A dropped prefix falls back to the closest kept covering prefix, if any, so a hijacked more specific resolves to its legitimate aggregate. A VRP for AS0 never validates anything. Prefixes longer than /24 in IPv4 or /48 in IPv6, shorter than /8 or /16, IPv6 prefixes outside `2000::/3`, and the Teredo (`2001::/32`) and 6to4 (`2002::/16`) ranges are ignored. The organization is the description of the AS in `asn.txt`, or its handle when it has none.

## Writing the files

`mmdb-writer` implements the [MaxMind DB format](https://maxmind.github.io/MaxMind-DB/) with 32-bit records in an IPv6 tree. IPv4 networks live under `::/96` and are aliased from `::ffff:0:0/96` and `2002::/16`, as MaxMind does, so IPv4-mapped and 6to4 addresses resolve to their IPv4 record. Identical records are stored once, and sibling networks with the same record are merged into their parent. Every file is checked by the tests with the `maxminddb` reader, including its `verify` method.

## Layout

| Crate | Role |
|---|---|
| `model` | shared types: registries, delegations, AS names, VRPs, routes |
| `fetch` | cached HTTP downloads with their metadata (`<file>.meta.json`) |
| `src-delegated` | parser for the RIR delegated-extended files |
| `src-asnames` | parser for `asn.txt` |
| `src-rpki` | parser for VRP JSON (rpki-client and Routinator formats) and RFC 6811 validator |
| `src-bgp` | MRT RIB reader, aggregating origins per prefix |
| `mmdb-writer` | MaxMind DB writer |
| `merge` | builds the country and ASN databases from the parsed sources |
| `cli` | the `geoip-builder` binary |

## Comparison with GeoLite2

On 2026-10-01, against GeoLite2 Country of 2026-09-29 and GeoLite2 ASN of 2026-10-01:

| Database | Family | Agree | Disagree | Only in GeoLite2 | Only in ours |
|---|---|---|---|---|---|
| Country | IPv4 | 95.08 % | 4.92 % | 0.00 % | 0.04 % |
| Country | IPv6 | 98.78 % | 1.21 % | 0.02 % | 0.02 % |
| ASN | IPv4 | 99.71 % | 0.26 % | 0.03 % | 0.32 % |
| ASN | IPv6 | 97.24 % | 2.51 % | 0.25 % | 2.93 % |
| ASN, `--rpki-valid-only` | IPv4 | 64.84 % | 0.22 % | 34.94 % | 0.26 % |
| ASN, `--rpki-valid-only` | IPv6 | 72.61 % | 2.46 % | 24.93 % | 0.39 % |

Country disagreements are almost all ranges registered in the United States and located elsewhere by MaxMind (cloud providers). About 27 % of announced prefixes have no covering ROA, among them those of large networks such as AS749, AS7018, AS3356 and AS174: they make up the gap of `--rpki-valid-only`.
