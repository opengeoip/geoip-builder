# Development

The project is a Cargo workspace; the toolchain comes from the distribution (`apt install cargo`). The minimum supported Rust version is 1.88.

```sh
cargo build --release
cargo test
cargo clippy --all-targets
cargo fmt --check
```

## Crates

| Crate | Role |
|---|---|
| `model` | shared types (registries, delegations, AS names, VRPs, routes, locations), longest-prefix map and special-purpose ranges |
| `fetch` | cached HTTP downloads with their metadata (`<file>.meta.json`) |
| `src-delegated` | parser for the RIR delegated-extended files |
| `src-asnames` | parser for `asn.txt` |
| `src-rpki` | parser for VRP JSON (rpki-client and Routinator formats) and RFC 6811 validator |
| `src-bgp` | MRT RIB reader, aggregating origins per prefix |
| `src-rpsl` | extracts geofeed references from RPSL `inetnum` and `inet6num` objects |
| `src-geofeed` | RFC 8805 parser and polite parallel crawler |
| `src-arin` | reader for the precompiled ARIN NetRanges and for ARIN RDAP network objects |
| `src-atlas` | reader for the RIPE Atlas probe archive |
| `mmdb-writer` | MaxMind DB writer |
| `merge` | origin selection, geofeed authorization, and the country, city and ASN databases |
| `cli` | the `geoip-builder` binary |

The code carries no comments: explanations live in these documents.

## Contributing

Changes go through pull requests on `main`, merged by squash. The pull request title becomes the commit message and must follow [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `docs:`, `refactor:`, `ci:`, `chore:`…, with `!` for a breaking change): release versions and the changelog are derived from it.

## Continuous integration

`.github/workflows/ci.yml` runs on every pull request and on `main`: formatting, Clippy with warnings denied, tests, a check with the minimum supported Rust version, a static `x86_64-unknown-linux-musl` build, and the pull request title. The `CI result` job aggregates them and is the only required check.

## Releases

Releases are driven by [release-plz](https://release-plz.dev/), configured in `release-plz.toml`:

1. every push to `main` updates a release pull request that bumps the workspace version from the Conventional Commits since the last tag and updates `CHANGELOG.md`;
2. merging that pull request creates the `vX.Y.Z` tag and the GitHub release;
3. `.github/workflows/release.yml` then builds static binaries for `x86_64` and `aarch64` (musl), attaches them to the release with a `SHA256SUMS` file and build provenance attestations, and publishes a multi-architecture image to `ghcr.io/opengeoip/geoip-builder` (tags `X.Y.Z`, `X.Y` and `latest`), also attested.

The image is based on distroless `static` (non-root) and runs in `/work`, where the geofeed catalog is installed; mount a volume on `/work/data` and `/work/out`:

```sh
docker run --rm -v "$PWD/data:/work/data" -v "$PWD/out:/work/out" ghcr.io/opengeoip/geoip-builder run
```

release-plz authenticates with a GitHub App of the organisation, so that its pull requests run the CI and its releases trigger the release workflow. The App needs the *Contents* and *Pull requests* repository permissions (read and write) and must be installed on the repository; its ID is the `RELEASE_APP_ID` variable and its private key the `RELEASE_APP_PRIVATE_KEY` secret of the `release` environment, restricted to `main`. Until `RELEASE_APP_ID` is set, the release jobs are skipped.

## Dependency updates

[Renovate](https://docs.renovatebot.com/) (`renovate.json`) opens grouped pull requests every Monday morning for Rust dependencies, GitHub Actions (pinned by commit SHA) and the container base image (pinned by digest), and refreshes `Cargo.lock`.

