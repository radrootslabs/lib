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

The current ordinary Client supports explicit in-memory preview and native
public-cache resource ownership. Authenticated workspace commands and the full
durable commerce workflow remain in implementation.

```rust
use radroots::{Client, StartMode};

async fn preview() -> radroots::Result<()> {
    let client = Client::preview();
    client.start(StartMode::LocalOnly).await?;
    client.close().await?;
    Ok(())
}
```

See [the ordinary package](crates/radroots/README.md) for current implemented
boundaries and explicit native opening. Construction does not start networking,
restore authority or replay business writes.

### Getting started

> **Status:** Alpha (`0.1.0-alpha`)
> Still a `draft`. Not recommended for use before `0.1.0` is published.

The Radroots libraries are a Rust workspace with a small set of native build
dependencies. Governed repository commands use `cargo extbuild`; see
[`BUILD.md`](BUILD.md) for the complete native setup guide.

#### Governed development

After installing the native dependencies below and configuring extbuild:

```sh
git clone https://radroots.dev/git/lib.git && cd lib

cargo extbuild doctor
cargo extbuild run -- cargo check --workspace --locked
```

Checked-in Nix expressions are deferred compatibility surfaces. Nix
evaluation and Nix-derived packages, apps, checks, development shells, NixOS
modules, and OCI outputs are not current qualification prerequisites.

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
