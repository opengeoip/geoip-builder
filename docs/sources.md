# Sources

Everything is downloaded in bulk by `fetch`. Nothing is queried per address.

| Data | Source | Used for |
|---|---|---|
| Registry statistics | [delegated-extended files](https://www.nro.net/about/rirs/statistics/) of the five RIRs | country of every allocated block |
| Registry databases | RPSL dumps of RIPE NCC, APNIC, AFRINIC and LACNIC | country of sub-allocations, geofeed references |
| ARIN geofeed references | daily file of [geofeed-finder](https://github.com/massimocandela/geofeed-finder) | geofeed references in ARIN space |
| Geofeed catalog | latest release of [opengeoip/geofeeds](https://github.com/opengeoip/geofeeds) | the geofeeds to download |
| Geofeeds | the URLs of the catalog | location declared by each operator |
| BGP | [RIPE RIS](https://ris.ripe.net/) routing table dumps | origin AS of every prefix |
| RPKI | [rpki-client](https://console.rpki-client.org/) export | route origin validation |
| AS names | [RIPE NCC list](https://ftp.ripe.net/ripe/asnames/asn.txt) | organization of each AS |
| Network types | [PeeringDB](https://www.peeringdb.com/) | hosting providers, `evaluate`, `candidates` |
| Users per AS | [APNIC Labs](https://stats.labs.apnic.net/aspop) | hosting providers |
| Ground truth | [RIPE Atlas](https://atlas.ripe.net/) probe archive and its tags | `evaluate` |
| Probes to ignore | [violating_ripe_probes](https://github.com/kizhikevich/violating_ripe_probes) | `evaluate` |
| Anycast prefixes | [LACeS census](https://github.com/ut-dacs/anycast-census) | `evaluate` |

## ARIN

ARIN does not publish its database freely: a dump needs a signed [Bulk Whois](https://www.arin.net/reference/research/bulkwhois/) agreement, and its RDAP terms forbid compiling the database. Its geofeed references therefore come from the file geofeed-finder publishes every day. Every `fetch` and `discover` checks a sample of its records against ARIN RDAP, one request per second (`--arin-check-sample`, 10 by default). `fetch` reports any difference; `discover` fails, so the published catalog keeps its previous version rather than taking a file that disagrees with ARIN. A Bulk Whois dump would replace that file.
