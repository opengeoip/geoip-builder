# Accuracy

## Against RIPE Atlas probes

`evaluate` checks the country of each connected RIPE Atlas probe address. It leaves out probes listed as misreporting their location, anycast addresses, and probes whose location Atlas set from IP geolocation (tags `system-auto-geoip-country` and `system-auto-geoip-city`), since their reference comes from another GeoIP database. `--keep-suspicious` keeps them all.

Results are split by the network type the probe's AS declares in PeeringDB, and by continent. `--json` writes them all to a file.

On the 2026-10-08 release:

| Probes | IPv4 | IPv6 |
|---|---|---|
| all | 96.4 % | 81.6 % |
| access networks (Cable/DSL/ISP) | 99.0 % | 99.4 % |
| transit networks (NSP) | 97.1 % | 93.0 % |
| content and hosting | 83.2 % | 86.6 % |
| without an AS in Atlas | 6 probes | 3.4 % |

| Continent | IPv4 | IPv6 |
|---|---|---|
| Europe | 97.1 % (6 808 probes) | 82.9 % (4 450) |
| North America | 97.4 % (1 853) | 78.2 % (1 188) |
| Asia | 93.1 % (1 250) | 79.0 % (729) |
| South America | 91.5 % (375) | 82.0 % (239) |
| Oceania | 95.6 % (344) | 82.1 % (240) |
| Africa | 93.9 % (293) | 80.5 % (164) |

Six probes in ten are in Europe, so the overall figure mostly reflects Europe. The IPv6 figure is held down by about 1 000 probe addresses that Atlas sees without an AS: they are not announced in BGP and no database places them. Most of the IPv4 gap comes from cloud and hosting networks that publish no geofeed, such as Oracle Cloud or Microsoft Azure.

## Against GeoLite2

Share of address space where both databases agree, on 2026-10-01 (`compare`):

| Database | IPv4 | IPv6 |
|---|---|---|
| Country | 97.1 % | 98.2 % |
| ASN | 99.7 % | 97.2 % |
| ASN, RPKI-valid only | 64.8 % | 72.6 % |

Registry statistics alone agree on 95.1 % of IPv4; sub-allocations and geofeeds bring it to 97.1 %. Remaining differences are mostly cloud ranges registered in one country and used in another. In IPv6, some blocks are assigned in full to a customer abroad, where GeoLite2 keeps the holder's country. About a quarter of announced prefixes have no RPKI record, which explains the RPKI-valid-only gap.
