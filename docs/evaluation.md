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

Two measures, because neither is enough alone.

### Against RIPE Atlas probes

`evaluate out/anonymous-ip.mmdb` checks the hosting flag against probes whose owners tagged them as hosted (`datacentre`, `vps`…) or on an access network (`home`, `fibre`, `cable`…). Probes with both kinds of tags, or neither, are left out. So are hosted probes in an AS that PeeringDB lists as an access, research, non-profit or government network, and not as content: they sit in the datacentre of an ISP or a university, which is not a hosting provider.

On 2026-10-08:

| Probes | IPv4 | IPv6 |
|---|---|---|
| flagged probes that are hosted | 81.2 % | 79.9 % |
| probes not flagged that are on an access network | 96.2 % | 95.3 % |
| hosted probes that are flagged | 65.8 % | 69.0 % |
| hosted / access probes | 485 / 4 231 | 352 / 2 252 |
| hosted probes in access or research networks, left out | 145 | 135 |

Atlas has about nine access probes for each hosted one, so these figures describe the probes, not the address space. Tags describe where the probe is, not who holds the address: a home probe behind a VPN shows up as a false positive.

### Audit of the address space

`audit-sample` draws IPv4 addresses at random among routed addresses, 100 flagged and 100 not flagged, so that each block weighs by its size. Each address is then judged by hand from its reverse DNS name and its registry object. An address counts as hosting when servers in a datacentre use it: cloud, hosting, CDN, or a content provider's own servers. Access networks, companies, administrations, universities and ISP infrastructure do not count. When the evidence is not enough, the verdict is `unknown`. `audit-score` computes the shares with 95 % Wilson intervals and estimates how much of the hosting address space is flagged.

The audit of 2026-10-09 is in [audit/hosting-2026-10-09.csv](audit/hosting-2026-10-09.csv):

| IPv4 addresses | Share | 95 % interval |
|---|---|---|
| flagged addresses that are hosting | 86.1 % | 76.8 – 92.0 % |
| addresses not flagged that are not hosting | 93.8 % | 87.0 – 97.1 % |
| hosting addresses that are flagged | 72.9 % | |

The 25 `unknown` verdicts are left out. 21 of them are flagged, mostly in the address space of Lumen, Cogent and Apple: counted as not hosting, the first share would drop to 68 %. All 11 flagged addresses judged not hosting come from the population rule (business access, dial-up and mobile pools, corporate and government networks of `NSP` or `Enterprise` ASes); none comes from a `Content` declaration. The 6 hosting addresses missed are in ASes that PeeringDB does not type, such as Akamai's AS16625, Google's AS396982 or Rackspace.

## Against GeoLite2

Share of address space where both databases agree, on 2026-10-01 (`compare`):

| Database | IPv4 | IPv6 |
|---|---|---|
| Country | 97.1 % | 98.2 % |
| ASN | 99.7 % | 97.2 % |
| ASN, RPKI-valid only | 64.8 % | 72.6 % |

Registry statistics alone agree on 95.1 % of IPv4; sub-allocations and geofeeds bring it to 97.1 %. Remaining differences are mostly cloud ranges registered in one country and used in another. In IPv6, some blocks are assigned in full to a customer abroad, where GeoLite2 keeps the holder's country. About a quarter of announced prefixes have no RPKI record, which explains the RPKI-valid-only gap.
