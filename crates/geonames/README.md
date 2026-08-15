# radroots_geonames

`radroots_geonames` is the concrete GeoNames data provider for Radroots. It
owns a pinned asset specification, explicit integrity-checked acquisition,
read-only database lifecycle, and deterministic forward, reverse, feature, and
country queries through provider-owned types.

The crate does not choose cache or runtime paths, download during construction,
create an async runtime, spawn a crate-owned task, install a timer, expose
SQLite or HTTP client types, or define a generic geocoder SPI. Publication
remains disabled during the `0.1.0-alpha` refactor.

The authoritative package charter is the
[`radroots_geonames` section of the Release V1 specification](../../contracts/crates/release_v1/radroots_crates_release_v1.toml).
The reviewed Rust surface is recorded in the
[public API baseline](../../contracts/api_baselines/radroots_geonames.txt).

## Prepare a query without I/O

Construction is validated and inert:

```rust
use radroots_geonames::{Point, Query};

let locality = Query::locality("Victoria")?
    .with_region("BC")?
    .with_country("CA")?
    .with_limit(5)?;
assert_eq!(locality.limit(), 5);

let reverse = Query::reverse(Point::new(48.4284, -123.3656)?)
    .with_radius_degrees(0.25)?;
assert_eq!(reverse.limit(), 1);
# Ok::<(), radroots_geonames::Error>(())
```

A runnable inert example is available at
[`examples/prepare_query.rs`](examples/prepare_query.rs).

## Asset identity and acquisition

[`asset::official_asset_spec`](crate::asset::official_asset_spec) returns the
byte-pinned official version, file name, HTTPS source and authority, exact
length, and SHA-256. Hosts may construct another [`AssetSpec`] only with one
safe destination file name and an HTTPS URL whose authority matches exactly;
userinfo, query strings, fragments, non-default ports, and plaintext sources
are rejected.

[`asset::inspect`](crate::asset::inspect) is passive. It reports missing,
available, or invalid bytes and never repairs or downloads them.
[`download::acquire`](crate::download::acquire) must be called explicitly with
an existing host-selected directory and a host-owned [`download::Fetcher`].
The crate bounds the stream before writing, stages in that same directory,
checks exact size and SHA-256, synchronizes it, and atomically replaces the
destination under an advisory lock. Symlink destinations fail closed.

The fetcher owns DNS, network deadlines, and cancellation. Its typed failure
phase cannot carry source URLs, credentials, or upstream error strings across
the public boundary. The final rename is the local acquisition commit point;
an interruption before it leaves the existing destination unchanged.

## Database and query behavior

[`Geocoder::open`] accepts only an explicit regular file matching its
[`AssetSpec`]. It opens SQLite read-only and query-only, runs an integrity
check, and validates the required `geonames` and `coordinates` table columns.
Opening, querying, and closing are caller-driven async operations. The caller
provides the async runtime; this crate does not create one or spawn crate-owned
tasks. Use [`Geocoder::close`] when an explicit terminal close result is
required.

[`Geocoder::query`] supports:

- structured locality filters by locality, region, and country;
- comma-separated free-form locality input;
- exact GeoNames feature identifiers;
- bounded reverse lookup with antimeridian and polar handling; and
- deterministic country lists with provider-derived center points.

Every candidate order has explicit tie-breakers. Numeric or text SQLite
administrative identifiers become opaque strings at the private row boundary.
Candidate, country, point, query, and result fields remain private and are
read through accessors.

## Errors, serialization, and side effects

[`Error`] exposes stable package-owned categories without paths, SQL, hashes,
URLs, credentials, SQLx errors, or fetch-client errors. Host diagnostics
should add their own path and transport context only at an access-controlled
application boundary.

The package defines no Cargo features and no stable serialized form. Persist
host configuration in a host-owned versioned contract, then reconstruct
`AssetSpec` and `Query` through validating constructors. Merely importing the
crate, obtaining the official specification, or constructing a query performs
no network, filesystem, database, runtime, clock, or process-global work.

## Intended consumers

- `radroots_sdk` composes GeoNames as an explicit optional capability.
- CLI and geocoding applications may acquire and query an asset directly.
- Ordinary applications normally use the curated `radroots` package.

## Copyright

Except as otherwise noted, all files in the `radroots_geonames` distribution
are copyright (c) 2025 Tyson Lupul. See `LICENSE` for usage, redistribution,
and warranty terms.
