#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root_dir"

cargo check -q
cargo test -q -p xtask
cargo run -q -p xtask -- sdk validate

required_file="$(mktemp)"
trap 'rm -f "$required_file"' EXIT
cargo run -q -p xtask -- sdk coverage required-crates >"$required_file"

rm -rf target/coverage
mkdir -p target/coverage

while IFS= read -r crate; do
  [ -n "$crate" ] || continue
  safe_crate="${crate//-/_}"
  out_dir="target/coverage/${safe_crate}"
  mkdir -p "$out_dir"

  cargo run -q -p xtask -- sdk coverage run-crate --crate "$crate" --out "$out_dir"
  cargo run -q -p xtask -- sdk coverage report \
    --scope "${crate}" \
    --summary "${out_dir}/coverage-summary.json" \
    --lcov "${out_dir}/coverage-lcov.info" \
    --out "${out_dir}/gate-report.json" \
    --policy-gate
done <"$required_file"

cargo run -q -p xtask -- sdk coverage refresh-summary \
  --reports-root target/coverage \
  --out target/coverage/coverage-refresh.tsv \
  --status-out target/coverage/coverage-refresh-status.tsv

cargo run -q -p xtask -- sdk release preflight
echo "release preflight complete"
