# Usage

```sh
geoip-builder run --data-dir data --out-dir out
geoip-builder lookup out/asn.mmdb 1.1.1.1 2a01:cb00::1
geoip-builder compare --kind country out/country.mmdb GeoLite2-Country.mmdb
```

- `fetch` downloads every source into `--data-dir`, runs `discover`, then downloads every geofeed of the catalog into `--data-dir/geofeeds`, with conditional requests (`If-None-Match`, `If-Modified-Since`), so running it often costs little when nothing changed. Failed geofeeds are listed in `geofeeds/failures.tsv` and keep their previous copy, if any.
- `discover` reads the RPSL dumps and the ARIN NetRanges already in `--data-dir` and rewrites the geofeed catalog (`--geofeeds`, default `catalog/geofeeds.csv`) with every geofeed reference they contain, keeping the rows added by hand.
- `build` reads only `--data-dir` and the geofeed catalog and writes `country.mmdb`, `city.mmdb` and `asn.mmdb` into `--out-dir`. A build never touches the network, so it can be replayed on a saved data directory.
- `run` is `fetch` followed by `build`.
- `--rpki-valid-only` restricts the ASN database to RPKI-valid routes (see [databases](databases.md)).
- a source that fails to download keeps its previous copy; `fetch` reports it and exits with an error once everything else is done.
- `--geofeeds` (default `catalog/geofeeds.csv`) is the geofeed catalog (see [databases](databases.md)).
- `--geofeed-workers` (default 32) sets how many hosts are crawled in parallel; the URLs of one host are fetched one after the other, 500 ms apart, with one retry after a `429`.
- `--collector` (repeatable, default `rrc00`) selects the RIPE RIS collectors whose RIB dumps are used. `--vrps-url` points to another VRP export, such as a local Routinator.
- `lookup` prints the network and record matching each address.
- `evaluate` checks one or more databases against the RIPE Atlas probes: for every connected probe with a public address, the country it reports versus the country each database gives, per address family, for all probes, anchors (in datacentres) and other probes separately, with the most frequent errors and, for the first two databases, which one is right when they disagree. Probes listed as misplaced by [violating_ripe_probes](https://github.com/kizhikevich/violating_ripe_probes) and addresses inside an anycast prefix of the [LACeS census](https://github.com/ut-dacs/anycast-census) are left out, unless `--keep-suspicious` is set. `--only <country>` narrows the probes.
- `coverage` lists, for every AS of the ASN database, the announced address space that no accepted geofeed covers, to find the networks whose geofeed is missing from the catalog. It writes a CSV (`--output`, default `out/geofeed-coverage.csv`; columns `asn`, `name`, `country`, `ipv4_announced`, `ipv4_without_geofeed`, `ipv4_share_without_geofeed`, `ipv6_announced_48`, `ipv6_without_geofeed_48`) sorted by IPv4 space without a geofeed, and prints the first `--top` (default 20).
- `candidates` helps grow the catalog. It groups by origin AS the reliable Atlas probes that the built country database (`--country`, `--asn`) places in the wrong country, joins the share of their space without a geofeed (from `coverage`) and their website from PeeringDB, and writes them to `--output` (default `out/geofeed-candidates.csv`), most misplaced first. For the first `--probe` (default 50) of them, it tries the usual geofeed locations on that website (`/geofeed.csv`, `/geofeed`, `/geofeed.txt`, `/.well-known/geofeed`, `geofeed.<domain>`…) and reports any file that parses as an RFC 8805 geofeed of at least 10 entries; `--add` appends the new ones to the catalog as unanchored `manual` rows, whose publisher is then checked against BGP like any other.
- `compare` measures how far a database is from a reference one, weighted by IPv4 addresses and IPv6 /48 networks, with the top disagreements and the largest ranges behind each of them. `--only <key>` (a country code, or `AS<n>`) restricts it to the ranges where either database has that value.

A first `fetch` takes about 8 minutes: 5 for the bulk files (about 900 MB), 3 for the geofeeds. `build` takes about 50 seconds and peaks at about 1.5 GB of memory.
