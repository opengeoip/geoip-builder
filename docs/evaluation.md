# Evaluation

## Against RIPE Atlas probes

On 2026-10-02 (`evaluate`, connected probes with a public address):

| Database | IPv4 correct | IPv6 correct |
|---|---|---|
| ours | 95.34 % | 79.77 % |
| GeoLite2 Country | 97.99 % | 80.00 % |
| ours, anchors only | 91.34 % | 92.37 % |
| GeoLite2 Country, anchors only | 93.41 % | 90.15 % |

Probes listed as misplaced and anycast addresses are left out. About 16 % of the IPv6 probe addresses have no answer in either database. Most of the remaining IPv4 gap comes from hosting and cloud networks that publish no geofeed (Oracle Cloud, Microsoft Azure) or leave ranges out of theirs.

## Against GeoLite2

GeoLite2 Country of 2026-09-29 and GeoLite2 ASN of 2026-10-01, weighted by address space (`compare`), on 2026-10-01, country rows updated on 2026-10-02:

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
