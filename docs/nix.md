# Nix

This workspace uses Nix as the canonical development and CI environment contract.

## Install Nix

macOS:

```bash
sh <(curl --proto '=https' --tlsv1.2 -L https://nixos.org/nix/install)
```

Linux with systemd:

```bash
sh <(curl --proto '=https' --tlsv1.2 -L https://nixos.org/nix/install) --daemon
```

Enable flakes for your user:

```bash
mkdir -p ~/.config/nix
cat > ~/.config/nix/nix.conf <<'EOF'
experimental-features = nix-command flakes
accept-flake-config = true
EOF
```

## Optional direnv

Install `direnv` and `nix-direnv` if you want the shell to load automatically when you enter the repo.

```bash
brew install direnv
echo 'eval "$(direnv hook zsh)"' >> ~/.zshrc
nix profile install nixpkgs#nix-direnv
mkdir -p ~/.config/direnv
echo 'source $HOME/.nix-profile/share/nix-direnv/direnvrc' >> ~/.config/direnv/direnvrc
```

After that:

```bash
direnv allow
```

## Enter The Shell

Default shell:

```bash
nix develop
```

Coverage or release shell:

```bash
nix develop .#coverage
nix develop .#release
```

The shells provide:

- Rust `1.92.0` with `wasm32-unknown-unknown`
- pinned nightly cargo for coverage
- `wasm-pack`
- `cargo-llvm-cov`
- `pkg-config`
- `clang` and `libclang`
- `libsodium`

## Command Map

Pure flake checks:

```bash
nix flake check
```

Repo-aware command apps:

```bash
nix run .#fmt
nix run .#check
nix run .#contract
nix run .#export-ts
nix run .#coverage-report
nix run .#wasm-builds
nix run .#release-preflight
nix run .#validate-sdk-typescript -- ./sdk-typescript
nix run .#publish-dry-run
nix run .#publish-crates -- --dry-run
```

`nix flake check` is intentionally limited to pure surfaces:

- Nix, shell, and TOML formatting through `treefmt`
- Rust formatting through `cargo fmt --check`
- pure cargo check/test derivations for the contract crate set
- repo guards that can run without cargo registry network access

Repo-aware flows stay behind `nix run` apps because they need a real checkout:

- `sdk export-ts` writes into repo-local `target/`
- sdk sync validation runs `bun` against a checked-out `sdk-typescript` repo path
- coverage refresh and release preflight produce repo-local artifacts
- wasm packaging writes package output directories
- publish commands read runtime tokens and the live checkout state

## First Verification

After installation:

```bash
nix flake check
nix run .#contract
nix run .#release-preflight
```

## Notes

- Flakes only see tracked files when the source is treated as a git checkout. If Nix appears to miss a new file, `git add` it first.
- Do not put secrets in `flake.nix`.
- `publish-crates.sh` reads `CARGO_REGISTRY_TOKEN` or `CRATES_IO_TOKEN` from your runtime environment.

## Deferred Infrastructure

Public binary-cache support is intentionally deferred until repository infrastructure is ready to own it. The next infrastructure-backed follow-up is to provision a public cache, publish its trust key in `flake.nix`, and wire CI to write to it.
