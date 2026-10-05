# geoip-builder

Builds MaxMind DB (`.mmdb`) files, compatible with the GeoLite2 Country, City and ASN schemas, from public bulk data only: no WHOIS, no RDAP, no rate-limited API.

## Usage

```sh
cargo build --release
target/release/geoip-builder run --data-dir data --out-dir out
target/release/geoip-builder lookup out/asn.mmdb 1.1.1.1 2a01:cb00::1
target/release/geoip-builder compare --kind country out/country.mmdb GeoLite2-Country.mmdb
```

- `fetch` downloads every source into `--data-dir`, runs `discover`, then downloads every geofeed of the catalog into `--data-dir/geofeeds`, with conditional requests (`If-None-Match`, `If-Modified-Since`), so running it often costs little when nothing changed. Failed geofeeds are listed in `geofeeds/failures.tsv` and keep their previous copy, if any.
- `discover` reads the RPSL dumps and the ARIN NetRanges already in `--data-dir` and rewrites the geofeed catalog (`--geofeeds`, default `catalog/geofeeds.csv`) with every geofeed reference they contain, keeping the rows added by hand.
- `build` reads only `--data-dir` and the geofeed catalog and writes `country.mmdb`, `city.mmdb` and `asn.mmdb` into `--out-dir`. A build never touches the network, so it can be replayed on a saved data directory.
- `run` is `fetch` followed by `build`.
- `--rpki-valid-only` restricts the ASN database to RPKI-valid routes (see below).
- a source that fails to download keeps its previous copy; `fetch` reports it and exits with an error once everything else is done.
- `--geofeeds` (default `catalog/geofeeds.csv`) is the geofeed catalog (see below).
- `--geofeed-workers` (default 32) sets how many hosts are crawled in parallel; the URLs of one host are fetched one after the other, 500 ms apart, with one retry after a `429`.
- `--collector` (repeatable, default `rrc00`) selects the RIPE RIS collectors whose RIB dumps are used. `--vrps-url` points to another VRP export, such as a local Routinator.
- `lookup` prints the network and record matching each address.
- `evaluate` checks one or more databases against the RIPE Atlas probes: for every connected probe with a public address, the country it reports versus the country each database gives, per address family, for all probes, anchors (in datacentres) and other probes separately, with the most frequent errors and, for the first two databases, which one is right when they disagree. Probes listed as misplaced by [violating_ripe_probes](https://github.com/kizhikevich/violating_ripe_probes) and addresses inside an anycast prefix of the [LACeS census](https://github.com/ut-dacs/anycast-census) are left out, unless `--keep-suspicious` is set. `--only <country>` narrows the probes.
- `coverage` lists, for every AS of the ASN database, the announced address space that no accepted geofeed covers, to find the networks whose geofeed is missing from the catalog. It writes a CSV (`--output`, default `out/geofeed-coverage.csv`; columns `asn`, `name`, `country`, `ipv4_announced`, `ipv4_without_geofeed`, `ipv4_share_without_geofeed`, `ipv6_announced_48`, `ipv6_without_geofeed_48`) sorted by IPv4 space without a geofeed, and prints the first `--top` (default 20).
- `candidates` helps grow the catalog. It groups by origin AS the reliable Atlas probes that the built country database (`--country`, `--asn`) places in the wrong country, joins the share of their space without a geofeed (from `coverage`) and their website from PeeringDB, and writes them to `--output` (default `out/geofeed-candidates.csv`), most misplaced first. For the first `--probe` (default 50) of them, it tries the usual geofeed locations on that website (`/geofeed.csv`, `/geofeed`, `/geofeed.txt`, `/.well-known/geofeed`, `geofeed.<domain>`…) and reports any file that parses as an RFC 8805 geofeed of at least 10 entries; `--add` appends the new ones to the catalog as unanchored `manual` rows, whose publisher is then checked against BGP like any other.
- `compare` measures how far a database is from a reference one, weighted by IPv4 addresses and IPv6 /48 networks, with the top disagreements and the largest ranges behind each of them. `--only <key>` (a country code, or `AS<n>`) restricts it to the ranges where either database has that value.

A first `fetch` takes about 8 minutes: 5 for the bulk files (about 900 MB), 3 for the geofeeds. `build` takes about 50 seconds and peaks at about 1.5 GB of memory.

## Sources

| Source | File | Used for |
|---|---|---|
| [RIR delegated-extended statistics](https://www.nro.net/about/rirs/statistics/) of AFRINIC, APNIC, ARIN, LACNIC and RIPE NCC | `delegated-<rir>` | country of every allocated or assigned IPv4 and IPv6 block |
| [RIPE NCC AS names](https://ftp.ripe.net/ripe/asnames/asn.txt), compiled from the five RIRs | `asn.txt` | AS organization names |
| [rpki-client VRP export](https://console.rpki-client.org/) | `vrps.json` | route origin validation |
| [RIPE RIS](https://ris.ripe.net/) RIB dumps (MRT) | `ris-<collector>.bview.gz` | origin AS of every announced prefix |
| RPSL dumps of RIPE NCC (`inetnum`, `inet6num`), APNIC (`inetnum`, `inet6num`), AFRINIC and LACNIC | `rpsl-*.gz` | country of sub-allocations and assignments, geofeed references ([RFC 9632](https://www.rfc-editor.org/rfc/rfc9632)) |
| [RIPE Atlas probe archive](https://ftp.ripe.net/ripe/atlas/probes/archive/) of the previous day | `atlas-probes.json.bz2` | ground truth for `evaluate` |
| [violating_ripe_probes](https://github.com/kizhikevich/violating_ripe_probes), latest list | `violating-probes.txt` | Atlas probes whose reported location is likely wrong, left out of `evaluate` |
| [LACeS anycast census](https://github.com/ut-dacs/anycast-census), latest IPv4 and IPv6 | `anycast-ipv4.csv`, `anycast-ipv6.csv` | anycast prefixes, left out of `evaluate` |
| [PeeringDB](https://www.peeringdb.com/) networks (`asn`, `name`, `website`, `info_type`) | `peeringdb-net.json` | websites probed by `candidates` |
| [RFC 8805](https://www.rfc-editor.org/rfc/rfc8805) geofeeds referenced by those objects | `geofeeds/<hash>.csv` | country, region, city and postal code declared by the operator |
| [`catalog/geofeeds.csv`](catalog/geofeeds.csv), written by `discover` and edited by hand | `geofeeds/<hash>.csv` | the list of geofeeds to crawl and their anchors |
| ARIN NetRanges carrying a geofeed reference, compiled daily over RDAP by [geofeed-finder](https://github.com/massimocandela/geofeed-finder) and published at `geofeeds.packetvis.com` | `arin-geofeed-inetnums.json` | geofeed references for space registered at ARIN |

ARIN publishes no `inetnum` dump without a [Bulk Whois](https://www.arin.net/reference/research/bulkwhois/) agreement, and its RDAP terms of use forbid compiling its database in bulk. The ARIN references therefore come from the file geofeed-finder publishes every day, which goes through the same RFC 9632 checks as the other registries. On every `fetch`, `--arin-check-sample` (default 10) NetRanges of that file, chosen anew each day, are looked up in ARIN RDAP one per second, and any difference between the file and the registry is reported. An ARIN Bulk Whois dump would replace that file.

## Country and city databases

Each record holds `country.iso_code`, `registered_country.iso_code` and `registry`. The city database adds `subdivisions[0].iso_code`, `city.names.en` and `postal.code` when a geofeed provides them.

The base layer is the RIR delegated statistics: `country` and `registered_country` are the country the RIR registered for the holder of the block. Blocks with a status other than `allocated` or `assigned`, and the `ZZ` code, are skipped. IPv4 ranges whose size is not a power of two are split into the minimal set of CIDR blocks.

The `country:` of RPSL `inetnum` and `inet6num` objects then refines it, for blocks a LIR sub-allocates or assigns to a customer in another country (an operator's foreign subsidiary, an overseas territory, a leased block). An object is used only when it is strictly more specific than a delegation of the same RIR: objects for the allocation itself carry a country typed by the LIR, while the delegated statistics carry the one checked by the RIR, and placeholder objects for space a RIR does not manage are left out. `EU` and `ZZ` are ignored and lower-case codes are accepted.

Accepted geofeed entries come last and override `country` with the location the operator declares. In every layer, `registered_country` and `registry` keep the values of the covering delegation, and entries are inserted from the least to the most specific prefix, so within a layer a more specific entry overrides its parent and each layer overrides the previous one.

### Geofeed authorization

A geofeed can claim any prefix, so an entry is only kept when its publisher is entitled to it:

- the geofeed must be referenced by an `inetnum`, `inet6num` or ARIN NetRange (a `geofeed:` attribute or a `Geofeed <url>` remark, over HTTP or HTTPS; like geofeed-finder, the common variants `Geofeed: <url>`, `geofeed:<url>` and `Comment: Geofeed <url>` are accepted, a bare URL is not), the most specific referencing object covering the entry must reference that same geofeed, as required by RFC 9632. An entry outside every object referencing its feed is dropped, and so is one inside a more specific object that references another feed;
- entries without a country code ("do not geolocate") and lines that do not parse are skipped. A region is kept only when it is an ISO 3166-2 code of the entry's country.

### Geofeed catalog

`catalog/geofeeds.csv` lists every geofeed the build uses, with a header line and three columns:

| Column | Content |
|---|---|
| `url` | geofeed URL |
| `network` | prefix of the registry object that references it, empty for an unanchored geofeed |
| `source` | `afrinic`, `apnic`, `arin`, `lacnic`, `ripencc` for discovered rows, `manual` for rows added by hand |

`discover` regenerates every non-`manual` row from the registries and keeps the `manual` rows untouched, so the catalog is versioned and its diffs show which references appeared or disappeared. Rows are sorted and one object referencing a geofeed gives one row per CIDR block it covers. Lines starting with `#` are comments.

A `manual` row with a `network` anchors its geofeed on that prefix exactly like a registry reference. A `manual` row without a `network` is for an operator that publishes a geofeed without referencing it from any registry object, so that no RFC 9632 discovery finds it. Since nothing vouches for such a feed, its publisher is inferred from the data, with no per-feed configuration:

- every entry is matched to the origin AS of the most specific announced route covering it (from the ASN database); entries covered by no route are dropped;
- the AS originating the most entries of the feed is its publisher, together with every other origin AS of the feed whose name in `asn.txt` shares a distinctive word with it (generic words such as `network`, `cloud` or `inc` do not count): `AMAZON-02` and `AMAZON-EXPANSION`, or `Akamai Connected Cloud` and `AKAMAI-ASN1`; a customer AS whose handle repeats its upstream's name (`CPC-COGENT-BLOCK` in Cogent's feed) is accepted too, which is harmless as long as it only announces that upstream's space;
- only entries announced by the publisher are kept, so ranges a customer announces itself (bring-your-own-IP) are dropped.

Unanchored geofeeds form a layer of their own, between `inetnum` countries and anchored geofeeds: an anchored geofeed always wins.

### Transport

Unlike RFC 9632, which requires HTTPS, `http://` references are accepted and certificates of geofeed servers are not verified: the authority of a feed comes from the registry object that references it, and the containment check above bounds what a tampered feed could claim to the prefixes of that object's holder. Certificates are still verified for every other source.

Signed geofeeds (RFC 9632 section 5) are not verified: very few are signed.

## ASN database

Each record holds `autonomous_system_number`, `autonomous_system_organization` and `rpki`, the [RFC 6811](https://www.rfc-editor.org/rfc/rfc6811) state of the kept route (`valid` or `not-found`).

RPKI-invalid routes are never kept. For every prefix seen in the RIB dumps:

1. the origin of each path is the last AS of its `AS_PATH` (merged with `AS4_PATH`); paths ending in an `AS_SET` of more than one AS are ignored;
2. origins are ranked by the number of distinct peers announcing them;
3. if at least one VRP covers the prefix, the best-ranked origin whose (prefix, origin) pair is `Valid` is kept, and the prefix is dropped when none is;
4. if no VRP covers the prefix (`NotFound`), the best-ranked origin is kept, unless `--rpki-valid-only` is set.

A dropped prefix falls back to the closest kept covering prefix, if any, so a hijacked more specific resolves to its legitimate aggregate. A VRP for AS0 never validates anything. Prefixes longer than /24 in IPv4 or /48 in IPv6, shorter than /8 or /16, IPv6 prefixes outside `2000::/3`, and the Teredo (`2001::/32`) and 6to4 (`2002::/16`) ranges are ignored. The organization is the description of the AS in `asn.txt`, or its handle when it has none.

## Special-purpose ranges

Ranges that are not globally reachable never carry data in either database, like in GeoLite2, even when a RIR delegation or a BGP announcement covers them. They come from the IANA [IPv4](https://www.iana.org/assignments/iana-ipv4-special-registry/) and [IPv6](https://www.iana.org/assignments/iana-ipv6-special-registry/) special-purpose registries, plus multicast and the reserved 240.0.0.0/4, and are listed in `crates/model/src/special.rs`:

- IPv4: this network (0/8), private-use (RFC 1918), shared address space (100.64/10), loopback, link local, IETF protocol assignments (192.0.0/24), documentation, benchmarking, the deprecated 6to4 relay anycast (192.88.99/24), multicast and reserved;
- IPv6: local-use NAT64 (64:ff9b:1::/48), discard-only, Teredo, benchmarking, deprecated ORCHID, documentation (2001:db8::/32, 3fff::/20), SRv6 SIDs, unique-local, link-local and multicast.

The few globally reachable entries of those registries (AS112, AMT, the well-known NAT64 prefix) are left alone. BGP routes inside a special range are dropped before validation, and the ranges are cleared from the tree after every insertion. `2002::/16` is then aliased back to the IPv4 tree, so a 6to4 address resolves to the record of the IPv4 address it embeds.

## Writing the files

`mmdb-writer` implements the [MaxMind DB format](https://maxmind.github.io/MaxMind-DB/) with 32-bit records in an IPv6 tree. IPv4 networks live under `::/96` and are aliased from `::ffff:0:0/96` and `2002::/16`, as MaxMind does, so IPv4-mapped and 6to4 addresses resolve to their IPv4 record. Identical records are stored once, and sibling networks with the same record are merged into their parent. Every file is checked by the tests with the `maxminddb` reader, including its `verify` method.

## Layout

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

## Comparison with GeoLite2

Against RIPE Atlas probes on 2026-10-02 (`evaluate`, connected probes with a public address):

| Database | IPv4 correct | IPv6 correct |
|---|---|---|
| ours | 95.34 % | 79.77 % |
| GeoLite2 Country | 97.99 % | 80.00 % |
| ours, anchors only | 91.34 % | 92.37 % |
| GeoLite2 Country, anchors only | 93.41 % | 90.15 % |

Probes listed as misplaced and anycast addresses are left out. About 16 % of the IPv6 probe addresses have no answer in either database. Most of the remaining IPv4 gap comes from hosting and cloud networks that publish no geofeed (Oracle Cloud, Microsoft Azure) or leave ranges out of theirs.

Against GeoLite2 Country of 2026-09-29 and GeoLite2 ASN of 2026-10-01, weighted by address space, on 2026-10-01 (country rows updated on 2026-10-02):

| Database | Family | Agree | Disagree | Only in GeoLite2 | Only in ours |
|---|---|---|---|---|---|
| Country | IPv4 | 97.07 % | 2.93 % | 0.00 % | 0.04 % |
| Country | IPv6 | 98.20 % | 1.78 % | 0.02 % | 0.07 % |
| Country, without `inetnum` countries | IPv4 | 96.57 % | 3.43 % | 0.00 % | 0.04 % |
| Country, without `inetnum` countries | IPv6 | 98.87 % | 1.11 % | 0.02 % | 0.07 % |
| Country, RIR statistics only | IPv4 | 95.08 % | 4.92 % | 0.00 % | 0.04 % |
| Country, RIR statistics only | IPv6 | 98.78 % | 1.21 % | 0.02 % | 0.02 % |
| ASN | IPv4 | 99.71 % | 0.26 % | 0.03 % | 0.32 % |
| ASN | IPv6 | 97.24 % | 2.51 % | 0.25 % | 2.93 % |
| ASN, `--rpki-valid-only` | IPv4 | 64.84 % | 0.22 % | 34.94 % | 0.26 % |
| ASN, `--rpki-valid-only` | IPv6 | 72.61 % | 2.46 % | 24.93 % | 0.39 % |

`inetnum` countries fix the foreign subsidiaries of operators (Orange in Spain, Iliad in Italy, OVH in the United Kingdom). In IPv6 they lower the agreement, because many /29 allocations are entirely assigned to a customer registered elsewhere, where GeoLite2 keeps the country of the holder. Most remaining country disagreements are cloud ranges registered in one country and used in another, by operators that publish no geofeed (Microsoft Azure) or leave ranges out of theirs. About 27 % of announced prefixes have no covering ROA, among them those of large networks such as AS749, AS7018, AS3356 and AS174: they make up the gap of `--rpki-valid-only`.
