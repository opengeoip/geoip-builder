# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.2](https://github.com/opengeoip/geoip-builder/compare/v0.2.1...v0.2.2) - 2026-10-07

### Fixed

- apply the country of inetnum objects covering a whole delegation ([#16](https://github.com/opengeoip/geoip-builder/pull/16))
- accept listed geofeed entries announced only as more-specifics ([#15](https://github.com/opengeoip/geoip-builder/pull/15))

### Other

- update Cargo.lock dependencies

## [0.2.1](https://github.com/opengeoip/geoip-builder/compare/v0.2.0...v0.2.1) - 2026-10-07

### Fixed

- apply the country of inetnum objects covering a whole delegation ([#16](https://github.com/opengeoip/geoip-builder/pull/16))
- accept listed geofeed entries announced only as more-specifics ([#15](https://github.com/opengeoip/geoip-builder/pull/15))

## [0.2.0](https://github.com/opengeoip/geoip-builder/compare/v0.1.2...v0.2.0) - 2026-10-07

### Added

- [**breaking**] fetch the geofeed catalog from the opengeoip/geofeeds releases ([#13](https://github.com/opengeoip/geoip-builder/pull/13))

## [0.1.2](https://github.com/opengeoip/geoip-builder/compare/v0.1.1...v0.1.2) - 2026-10-07

### Fixed

- name the command geoip-builder in its help and version output ([#11](https://github.com/opengeoip/geoip-builder/pull/11))

## [0.1.1](https://github.com/opengeoip/geoip-builder/compare/v0.1.0...v0.1.1) - 2026-10-06

### Other

- move command logic into analysis and pipeline crates, and tighten the documentation ([#6](https://github.com/opengeoip/geoip-builder/pull/6))

## [0.1.0](https://github.com/opengeoip/geoip-builder/releases/tag/v0.1.0) - 2026-10-06

### Added

- build GeoLite2-compatible country and ASN databases from RIR, RIS and RPKI data
- keep routes without ROA in the ASN database, still excluding RPKI-invalid ones
- keep special-purpose ranges empty and show the largest ranges behind each disagreement
- locate prefixes from RFC 8805 geofeeds authorized per RFC 9632, and build a city database
- restrict compare to one country or AS with --only
- refine countries with the country of RPSL sub-allocations and assignments
- evaluate databases against RIPE Atlas probes, and keep fetching when one source fails
- [**breaking**] take ARIN geofeed references from geofeed-finder, spot-checked over RDAP, instead of a hard-coded list of cloud feeds
- [**breaking**] split geofeed discovery into a versioned CSV catalog, with hand-added feeds whose publisher is inferred from BGP; accept HTTP and unverified TLS for geofeeds
- list per AS the announced space no geofeed covers
- clean the Atlas ground truth of misplaced probes and anycast and score anchors apart
- rank the ASes behind misplaced Atlas probes and probe their websites for unreferenced geofeeds

### Other

- license under GPL-3.0-or-later and move technical documentation out of the README
- add CI, release-plz releases with static binaries and a container image, and Renovate
