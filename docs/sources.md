# Sources

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
| [`catalog/geofeeds.csv`](../catalog/geofeeds.csv), written by `discover` and edited by hand | `geofeeds/<hash>.csv` | the list of geofeeds to crawl and their anchors |
| ARIN NetRanges carrying a geofeed reference, compiled daily over RDAP by [geofeed-finder](https://github.com/massimocandela/geofeed-finder) and published at `geofeeds.packetvis.com` | `arin-geofeed-inetnums.json` | geofeed references for space registered at ARIN |

ARIN publishes no `inetnum` dump without a [Bulk Whois](https://www.arin.net/reference/research/bulkwhois/) agreement, and its RDAP terms of use forbid compiling its database in bulk. The ARIN references therefore come from the file geofeed-finder publishes every day, which goes through the same RFC 9632 checks as the other registries. On every `fetch`, `--arin-check-sample` (default 10) NetRanges of that file, chosen anew each day, are looked up in ARIN RDAP one per second, and any difference between the file and the registry is reported. An ARIN Bulk Whois dump would replace that file.
