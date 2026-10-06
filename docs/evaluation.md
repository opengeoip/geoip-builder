# Accuracy

## Against RIPE Atlas probes

Share of probes whose country the database gets right, on 2026-10-02 (`evaluate`), leaving out probes known to misreport their location and anycast addresses:

| | IPv4 | IPv6 |
|---|---|---|
| geoip-builder | 95.3 % | 79.8 % |
| GeoLite2 | 98.0 % | 80.0 % |
| geoip-builder, datacentres only | 91.3 % | 92.4 % |
| GeoLite2, datacentres only | 93.4 % | 90.2 % |

About 16 % of probe IPv6 addresses are in neither database. Most of the IPv4 gap comes from cloud and hosting networks that publish no geofeed, such as Oracle Cloud or Microsoft Azure.

## Against GeoLite2

Share of address space where both databases agree, on 2026-10-01 (`compare`):

| Database | IPv4 | IPv6 |
|---|---|---|
| Country | 97.1 % | 98.2 % |
| ASN | 99.7 % | 97.2 % |
| ASN, RPKI-valid only | 64.8 % | 72.6 % |

Registry statistics alone agree on 95.1 % of IPv4; sub-allocations and geofeeds bring it to 97.1 %. Remaining differences are mostly cloud ranges registered in one country and used in another. In IPv6, some blocks are assigned in full to a customer abroad, where GeoLite2 keeps the holder's country. About a quarter of announced prefixes have no RPKI record, which explains the RPKI-valid-only gap.
