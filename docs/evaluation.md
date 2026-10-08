# Accuracy

## Against RIPE Atlas probes

`evaluate` checks the country of each connected RIPE Atlas probe address. It leaves out probes listed as misreporting their location, anycast addresses, probes whose location Atlas set from IP geolocation (tags `system-auto-geoip-country` and `system-auto-geoip-city`), since their reference comes from another GeoIP database, and addresses Atlas sees without an AS. Nineteen in twenty of the latter are private IPv6 addresses (`fc00::/7`) that probes report from their local network, which no database can place. `--keep-suspicious` keeps them all.

Results are split by the network type the probe's AS declares in PeeringDB, and by continent. `--json` writes them all to a file.

On the 2026-10-08 release:

| Probes | IPv4 | IPv6 |
|---|---|---|
| all | 96.2 % | 95.0 % |
| access networks (Cable/DSL/ISP) | 99.0 % | 99.3 % |
| transit networks (NSP) | 96.6 % | 92.4 % |
| content and hosting | 82.7 % | 86.2 % |

| Continent | IPv4 | IPv6 |
|---|---|---|
| Europe | 97.0 % (6 707 probes) | 95.5 % (3 803) |
| North America | 97.1 % (1 841) | 97.1 % (933) |
| Asia | 93.0 % (1 259) | 90.3 % (620) |
| South America | 91.4 % (360) | 93.8 % (194) |
| Oceania | 95.0 % (338) | 95.9 % (196) |
| Africa | 93.9 % (294) | 90.6 % (138) |

Six probes in ten are in Europe, so the overall figure mostly reflects Europe. Most of the gap comes from cloud and hosting networks that publish no geofeed, such as Oracle Cloud or Microsoft Azure.

## Hosting providers

`evaluate out/anonymous-ip.mmdb` checks the hosting flag against probes whose owners tagged them as hosted (`datacentre`, `vps`…) or on an access network (`home`, `fibre`, `cable`…). Probes with both kinds of tags, or neither, are left out.

On 2026-10-08:

| Probes | IPv4 | IPv6 |
|---|---|---|
| precision: flagged probes that are hosted | 81.2 % | 80.0 % |
| recall: hosted probes that are flagged | 50.8 % | 45.8 % |
| hosted / access probes | 630 / 4 235 | 533 / 2 768 |

Most missed probes are in ASes that declare themselves as `NSP` while carrying many VPN users, in research networks hosting servers, or in ASes absent from PeeringDB, such as Amazon's AS14618. Most false positives are home probes on small networks declared as `Content`. Tags are set by probe owners, so the reference is itself approximate.

## Against GeoLite2

Share of address space where both databases agree, on 2026-10-01 (`compare`):

| Database | IPv4 | IPv6 |
|---|---|---|
| Country | 97.1 % | 98.2 % |
| ASN | 99.7 % | 97.2 % |
| ASN, RPKI-valid only | 64.8 % | 72.6 % |

Registry statistics alone agree on 95.1 % of IPv4; sub-allocations and geofeeds bring it to 97.1 %. Remaining differences are mostly cloud ranges registered in one country and used in another. In IPv6, some blocks are assigned in full to a customer abroad, where GeoLite2 keeps the holder's country. About a quarter of announced prefixes have no RPKI record, which explains the RPKI-valid-only gap.
