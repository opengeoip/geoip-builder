# How the databases are built

## Country and city databases

Each record holds `country.iso_code`, `registered_country.iso_code` and `registry`. The city database adds `subdivisions[0].iso_code`, `city.names.en` and `postal.code` when a geofeed provides them.

The base layer is the RIR delegated statistics: `country` and `registered_country` are the country the RIR registered for the holder of the block. Blocks with a status other than `allocated` or `assigned`, and the `ZZ` code, are skipped. IPv4 ranges whose size is not a power of two are split into the minimal set of CIDR blocks.

The `country:` of RPSL `inetnum` and `inet6num` objects then refines it, for blocks a LIR sub-allocates or assigns to a customer in another country (an operator's foreign subsidiary, an overseas territory, a leased block). An object is used only when it is strictly more specific than a delegation of the same RIR: objects for the allocation itself carry a country typed by the LIR, while the delegated statistics carry the one checked by the RIR, and placeholder objects for space a RIR does not manage are left out. `EU` and `ZZ` are ignored and lower-case codes are accepted.

Accepted geofeed entries come last and override `country` with the location the operator declares. In every layer, `registered_country` and `registry` keep the values of the covering delegation, and entries are inserted from the least to the most specific prefix, so within a layer a more specific entry overrides its parent and each layer overrides the previous one.

## Geofeed authorization

A geofeed can claim any prefix, so an entry is only kept when its publisher is entitled to it:

- the geofeed must be referenced by an `inetnum`, `inet6num` or ARIN NetRange (a `geofeed:` attribute or a `Geofeed <url>` remark, over HTTP or HTTPS; like geofeed-finder, the common variants `Geofeed: <url>`, `geofeed:<url>` and `Comment: Geofeed <url>` are accepted, a bare URL is not), the most specific referencing object covering the entry must reference that same geofeed, as required by RFC 9632. An entry outside every object referencing its feed is dropped, and so is one inside a more specific object that references another feed;
- entries without a country code ("do not geolocate") and lines that do not parse are skipped. A region is kept only when it is an ISO 3166-2 code of the entry's country.

## Geofeed catalog

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

## Transport

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

Ranges that are not globally reachable never carry data in either database, like in GeoLite2, even when a RIR delegation or a BGP announcement covers them. They come from the IANA [IPv4](https://www.iana.org/assignments/iana-ipv4-special-registry/) and [IPv6](https://www.iana.org/assignments/iana-ipv6-special-registry/) special-purpose registries, plus multicast and the reserved 240.0.0.0/4, and are listed in [`crates/model/src/special.rs`](../crates/model/src/special.rs):

- IPv4: this network (0/8), private-use (RFC 1918), shared address space (100.64/10), loopback, link local, IETF protocol assignments (192.0.0/24), documentation, benchmarking, the deprecated 6to4 relay anycast (192.88.99/24), multicast and reserved;
- IPv6: local-use NAT64 (64:ff9b:1::/48), discard-only, Teredo, benchmarking, deprecated ORCHID, documentation (2001:db8::/32, 3fff::/20), SRv6 SIDs, unique-local, link-local and multicast.

The few globally reachable entries of those registries (AS112, AMT, the well-known NAT64 prefix) are left alone. BGP routes inside a special range are dropped before validation, and the ranges are cleared from the tree after every insertion. `2002::/16` is then aliased back to the IPv4 tree, so a 6to4 address resolves to the record of the IPv4 address it embeds.

## Writing the files

`mmdb-writer` implements the [MaxMind DB format](https://maxmind.github.io/MaxMind-DB/) with 32-bit records in an IPv6 tree. IPv4 networks live under `::/96` and are aliased from `::ffff:0:0/96` and `2002::/16`, as MaxMind does, so IPv4-mapped and 6to4 addresses resolve to their IPv4 record. Identical records are stored once, and sibling networks with the same record are merged into their parent. Every file is checked by the tests with the `maxminddb` reader, including its `verify` method.
