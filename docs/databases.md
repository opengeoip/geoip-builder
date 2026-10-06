# How it works

## Country and city

Each address gets the country of the most precise source that knows about it. The layers below are applied in order, each one overriding the previous for the ranges it covers:

1. **Registry statistics.** The country the RIR recorded for the holder of the block.
2. **Sub-allocations.** The country of registry objects (`inetnum`, `inet6num`) that a provider assigns to a customer, for example Orange's network in Spain inside a French allocation. Only objects strictly inside a block of the same RIR count: the country on the allocation itself is typed by the provider, while the RIR checks the one in its statistics.
3. **Unanchored geofeeds.** Geofeeds listed by hand in the catalog (see below).
4. **Registry geofeeds.** Geofeeds referenced from registry objects, the most reliable source.

`registered_country` always keeps the registry's value. The city database adds region, city and postal code when a geofeed provides them.

## Which geofeeds are trusted

A geofeed is a CSV file in which an operator says where its prefixes are ([RFC 8805](https://www.rfc-editor.org/rfc/rfc8805)). Anyone can publish one, so an entry is only kept when its publisher owns the prefix:

- **Registry geofeeds** follow [RFC 9632](https://www.rfc-editor.org/rfc/rfc9632): the registry object that covers the entry most precisely must point to that geofeed. An entry outside the referencing object, or inside a more precise object pointing elsewhere, is dropped. Common variants of the `Geofeed` remark are accepted.
- **Unanchored geofeeds** are published by operators who do not reference them from the registry, such as AWS or Google. Their publisher is inferred from BGP: the AS announcing most of the feed's prefixes, plus the ASes whose names share a distinctive word with it (`AMAZON-02` and `AMAZON-EXPANSION`). Entries announced by anyone else, such as a customer's own addresses, are dropped.

Geofeeds are downloaded over HTTP or HTTPS without checking certificates. Trust comes from the registry object, not from the transport, and the ownership check limits what a tampered file could claim. Every other source is downloaded with certificate checks.

## The geofeed catalog

[`catalog/geofeeds.csv`](../catalog/geofeeds.csv) lists every geofeed the build uses:

| Column | Content |
|---|---|
| `url` | the geofeed |
| `network` | the prefix of the registry object referencing it, empty for an unanchored geofeed |
| `source` | the registry it was found in, or `manual` for a row added by hand |

`discover` rewrites the registry rows and keeps the `manual` ones. The file is versioned, so its diffs show which geofeeds appeared or disappeared. To add a geofeed, add a line such as `https://example.net/geofeed.csv,,manual`.

## ASN

For each prefix in the BGP tables, the origin AS seen by the most peers is kept, unless RPKI says another AS should originate it. If an RPKI record covers the prefix, only a valid origin is kept; with no RPKI record, the most seen origin is kept, unless `--rpki-valid-only` is set. A dropped prefix falls back to the closest valid covering one, so a hijacked more specific resolves to its legitimate owner. Each record says whether its route is RPKI `valid` or `not-found`.

## Reserved ranges

Private, loopback, documentation, multicast and other special-purpose ranges from the IANA registries never get data, whatever the registries or BGP say. A 6to4 address (`2002::/16`) resolves to the IPv4 address it embeds.

## File format

The writer produces standard MaxMind DB files, checked in the tests with the reference `maxminddb` reader. IPv4-mapped addresses resolve like their IPv4 address.
