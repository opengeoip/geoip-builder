# Development

## Building and checking

The toolchain comes from the distribution (`apt install cargo`); the minimum Rust version is 1.88.

```sh
cargo build --release
cargo test
cargo clippy --all-targets
cargo fmt --check
```

The code has no comments: explanations live in these documents.

## Crates

| Crate | Role |
|---|---|
| `cli` | the `geoip-builder` command: arguments and reports |
| `pipeline` | the `fetch`, `discover` and `build` steps and the list of sources |
| `analysis` | the computations behind `evaluate`, `compare`, `coverage` and `candidates` |
| `merge` | builds the country, city and ASN databases |
| `mmdb-writer` | writes MaxMind DB files |
| `fetch` | cached downloads |
| `src-*` | one parser per source: `delegated`, `rpsl`, `arin`, `geofeed`, `bgp`, `rpki`, `asnames`, `atlas` |
| `model` | shared types and helpers |

## Contributing

Every change goes through a pull request, merged by squash. The pull request title becomes the commit message and follows [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `docs:`…, `!` for a breaking change): versions and the changelog are derived from it.

## CI

On every pull request: formatting, Clippy with warnings as errors, tests, a check with Rust 1.88, a static musl build and the pull request title. `CI result` is the single required check.

## Releases

[release-plz](https://release-plz.dev/) keeps a release pull request open with the next version and `CHANGELOG.md`. Merging it tags `vX.Y.Z` and publishes the release, which builds:

- static `x86_64` and `aarch64` Linux binaries, with `SHA256SUMS` and build provenance;
- a multi-architecture image, `ghcr.io/opengeoip/geoip-builder` (`X.Y.Z`, `X.Y`, `latest`), distroless and non-root, with the catalog in `/work`.

release-plz acts as the `opengeoip-bot` GitHub App, so that its pull requests and releases trigger the other workflows. Its client ID is the `BOT_CLIENT_ID` organisation variable, its private key the `BOT_PRIVATE_KEY` secret of the `release` environment, usable from `main` only. A release can be rebuilt by hand from *Actions → Release*.

## Dependencies

[Renovate](https://docs.renovatebot.com/) opens grouped pull requests every Monday for Rust crates, GitHub Actions (pinned by SHA) and the base image (pinned by digest).
