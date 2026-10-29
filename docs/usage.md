# Usage

## Building the databases

```sh
geoip-builder run                 # fetch, then build
geoip-builder fetch               # download every source into data/
geoip-builder build               # write out/country.mmdb, city.mmdb and asn.mmdb
geoip-builder build --csv         # also write the databases as CSV files in out/csv/
```

`fetch` downloads the bulk sources and the latest [geofeed catalog](https://github.com/opengeoip/geofeeds), then every geofeed it lists. It uses conditional requests, so a run where nothing changed is quick. When a download fails, the previous copy is kept and `fetch` exits with an error at the end. Failed geofeeds are listed in `data/geofeeds/failures.tsv`.

`build` never touches the network: it can be replayed on a saved data directory.

A first `fetch` takes about 8 minutes for about 900 MB. `build` takes about 50 seconds and up to 1.5 GB of memory.

| Option | Default | Meaning |
|---|---|---|
| `--data-dir` | `data` | where sources are downloaded |
| `--out-dir` | `out` | where databases are written (`build`, `run`) |
| `--csv` | off | also write the databases as CSV files (`build`, `run`) |
| `--geofeeds` | latest `opengeoip/geofeeds` release | the geofeed catalog, as a URL or a local file |
| `--collector` | `rrc00` | RIPE RIS collector to use, repeatable |
| `--vrps-url` | rpki-client export | RPKI data, for example from a local Routinator |
| `--rpki-valid-only` | off | keep only RPKI-valid routes in the ASN database |
| `--geofeed-workers` | `32` | geofeed hosts crawled in parallel |
| `--arin-check-sample` | `10` | ARIN records double-checked over RDAP on each `fetch` |

## CSV files

```sh
geoip-builder export-csv          # convert out/*.mmdb into out/csv/
```

`--csv` and `export-csv` write the databases in the layout of the GeoLite2 CSV downloads, with the same columns, so that existing import scripts work. Each file is compressed with gzip, about 22 MB in all (165 MB once decompressed with `gunzip` or read with `zcat`):

| File | GeoLite2 equivalent |
|---|---|
| `country-blocks-ipv4.csv.gz`, `country-blocks-ipv6.csv.gz` | `GeoLite2-Country-Blocks-IPv4.csv`, `-IPv6.csv` |
| `country-locations-en.csv.gz` | `GeoLite2-Country-Locations-en.csv` |
| `city-blocks-ipv4.csv.gz`, `city-blocks-ipv6.csv.gz` | `GeoLite2-City-Blocks-IPv4.csv`, `-IPv6.csv` |
| `city-locations-en.csv.gz` | `GeoLite2-City-Locations-en.csv` |
| `asn-blocks-ipv4.csv.gz`, `asn-blocks-ipv6.csv.gz` | `GeoLite2-ASN-Blocks-IPv4.csv`, `-IPv6.csv` |

The CSV files are read back from the `.mmdb` files, so both give the same answer for every address. The blocks point to the locations through `geoname_id`, but the databases have no GeoNames data: the identifiers are made up and do not match MaxMind's. A country gets a small number derived from its code (`FR` is 18002), a region or city a number above 16777216 derived from its country, region and city names. Both stay the same from one build to the next. Columns without a source stay empty: country and region names, coordinates, accuracy radius, metro code and time zone. `is_anonymous_proxy` and `is_satellite_provider` are always `0`, `is_anycast` is always empty.

## Building your own catalog

```sh
geoip-builder discover --fetch --manual manual.csv --output geofeeds.csv
geoip-builder check-catalog geofeeds.csv
```

`discover` lists every geofeed referenced in the registry dumps, adds the rows of `--manual`, and writes the catalog. `--fetch` downloads only the dumps it needs. Pass the result to `--geofeeds` to use it instead of the published one; a local catalog given to `fetch` is refreshed in place, keeping its `manual` rows. `check-catalog` validates catalog files, or `manual.csv` files with `--manual`.

## Looking up addresses

```sh
geoip-builder lookup out/city.mmdb 1.1.1.1 2a01:cb00::1
```

Prints the matching network and record for each address.

## Measuring accuracy

```sh
geoip-builder evaluate out/country.mmdb GeoLite2-Country.mmdb
geoip-builder compare --kind country out/country.mmdb GeoLite2-Country.mmdb
```

`evaluate` checks databases against the countries reported by RIPE Atlas probes, for IPv4 and IPv6, for all probes, anchors (datacentres) and home probes. It lists the most frequent errors and, for two databases, which one is right when they disagree. Probes known to report a wrong location and anycast addresses are left out, unless `--keep-suspicious` is set. `--only FR` limits it to one country, `--top` sets how many errors are listed.

`compare` measures how much two databases agree, weighted by IPv4 addresses and IPv6 /48s. It lists the largest disagreements and the ranges behind them. `--only FR` or `--only AS3215` limits it to one country or AS.

## Finding missing geofeeds

```sh
geoip-builder coverage
geoip-builder candidates --add-to manual.csv
```

`coverage` writes `out/geofeed-coverage.csv`: for each AS, how much of its announced space no geofeed covers.

`candidates` lists, AS by AS, the Atlas probes the country database gets wrong, with the network's website from PeeringDB, in `out/geofeed-candidates.csv`. For the first 50 (`--probe`), it tries the usual geofeed locations on the website, such as `/geofeed.csv` or `geofeed.<domain>`. `--add-to manual.csv` appends the geofeeds it finds to a manual list, each declared for the AS it was found for, ready for a pull request to [opengeoip/geofeeds](https://github.com/opengeoip/geofeeds).
