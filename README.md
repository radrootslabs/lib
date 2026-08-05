![Radroots libraries — foundations for local food networks](https://radroots.dev/files/assets/library/readme/radroots_library_banner.png?v=1.0.0)

Build local food networks that help farms and communities organize and communicate.

**What it does:** Radroots specifies the event and networking foundations for building interoperable local food networks, where governance and data ownership remain in the hands of the communities that run them.

**[Install](https://radroots.dev/docs/install)**
| [Docs](https://radroots.dev/docs)
| [Examples](https://radroots.dev/docs/examples)
| [Releases](https://radroots.dev/downloads/)
| [License](#license)

### Use cases
- Farmers post what they are growing and receive orders for upcoming harvests
- Food hubs organize group buying across farms, households and local businesses
- Farms and food hubs schedule drops and market days where buyers pick up orders
- Restaurants and local businesses arrange regular orders directly with nearby farms
- Local couriers carry orders from farms to homes, businesses and pickup points

### How to use
```rust
use radroots::prelude::*;

let client = Client::builder()
    .signer(signer)
    .transport(Nostr::relays(["wss://radroots.org"])?)
    .build()
    .await?;

let farm = client
    .seller(seller_account)?
    .farm(farm_id)?;

// Farmer creates a listing for freshly picked tomatoes
let receipt = farm
    .listings()
    .publish("Heirloom tomatoes")
    .id("fresh-tomatoes")
    .description("Locally grown and picked this morning.")
    .price(Price::cad_per_kg(4)?)
    .available(Quantity::kg(20)?)
    .pickup_in(Locality::city("Victoria", "BC", "CA")?)
    .send() // Validates, signs, saves locally, and attempts publication
    .await?;

// Log the published event address
println!("Published: {}", receipt.address());
…
```

### Getting started

> **Status:** Alpha (`0.1.0-alpha`)
> Still a `draft`. Not recommended for use before `0.1.0` is published.

The Radroots libraries are a Rust workspace with a small set of native build dependencies. Nix is recommended; the manual setup below covers the minimum requirements on macOS and Linux. See [`BUILD.md`](BUILD.md) for the complete build guide.

#### Nix (*recommended*)

The Nix development shell provides a configured environment on macOS and Linux:

```sh
git clone https://radroots.dev/git/lib.git && cd lib

nix develop
cargo check --workspace --locked
```

#### macOS and Linux

To setup manually, install [Rust](https://rustup.rs/), Git, LLVM/Clang with `libclang`, `pkg-config`, and `libsodium`.

On macOS with Homebrew:

```sh
brew install llvm pkgconf libsodium
export LIBCLANG_PATH="$(brew --prefix llvm)/lib"
```

On Debian or Ubuntu:

```sh
sudo apt install build-essential clang libclang-dev pkg-config libsodium-dev
```

Then clone and check the workspace:

```sh
git clone https://radroots.dev/git/lib.git && cd lib

export SODIUM_USE_PKG_CONFIG=1
cargo check --workspace --locked
```

On macOS, if `bindgen` cannot locate the system headers:

```sh
export SDKROOT="$(xcrun --sdk macosx --show-sdk-path)"
export BINDGEN_EXTRA_CLANG_ARGS="--sysroot=${SDKROOT}"
```

#### SP1 (*optional*)

SP1 is required to compile programs and generate proofs. Install Protocol Buffers and the SP1 toolchain:

```sh
# macOS
brew install protobuf

# Debian or Ubuntu
sudo apt install protobuf-compiler

curl -L https://sp1up.succinct.xyz | bash
sp1up
```

Run proof generation in release mode. CUDA proving is supported on compatible Linux systems, not on macOS. See [`BUILD.md`](BUILD.md) for the complete setup.

## Status

Radroots is under active development. The API is not yet stable and will change quite often.

## License

This repository is licensed under either:

- [Apache License 2.0](LICENSE-APACHE), or
- [MIT License](LICENSE-MIT),

at your option. 

Vendored or third-party material retains its own notices and license.
