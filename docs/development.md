# Development

The project is a Cargo workspace; the toolchain comes from the distribution (`apt install cargo`).

```sh
cargo build --release
cargo test
cargo clippy --all-targets
cargo fmt --check
```

## Crates

| Crate | Role |
|---|---|
| `model` | shared types (registries, delegations, AS names, VRPs, routes, locations), longest-prefix map and special-purpose ranges |
| `fetch` | cached HTTP downloads with their metadata (`<file>.meta.json`) |
| `src-delegated` | parser for the RIR delegated-extended files |
| `src-asnames` | parser for `asn.txt` |
| `src-rpki` | parser for VRP JSON (rpki-client and Routinator formats) and RFC 6811 validator |
| `src-bgp` | MRT RIB reader, aggregating origins per prefix |
| `src-rpsl` | extracts geofeed references from RPSL `inetnum` and `inet6num` objects |
| `src-geofeed` | RFC 8805 parser and polite parallel crawler |
| `src-arin` | reader for the precompiled ARIN NetRanges and for ARIN RDAP network objects |
| `src-atlas` | reader for the RIPE Atlas probe archive |
| `mmdb-writer` | MaxMind DB writer |
| `merge` | origin selection, geofeed authorization, and the country, city and ASN databases |
| `cli` | the `geoip-builder` binary |

The code carries no comments: explanations live in these documents.
