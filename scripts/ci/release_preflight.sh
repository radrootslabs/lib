#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root_dir"

cargo check -q
cargo test -q -p xtask
cargo run -q -p xtask -- sdk validate

required_file="$(mktemp)"
trap 'rm -f "$required_file"' EXIT
cargo run -q -p xtask -- sdk coverage required-crates > "$required_file"

mkdir -p target/coverage
printf "crate\tstatus\texec\tfunc\tbranch\treport\n" > target/coverage/coverage-refresh.tsv
printf "crate\tstatus\n" > target/coverage/coverage-refresh-status.tsv

while IFS= read -r crate; do
  [ -n "$crate" ] || continue
  safe_crate="${crate//-/_}"
  out_dir="target/coverage/${safe_crate}"
  mkdir -p "$out_dir"

  cargo run -q -p xtask -- sdk coverage run-crate --crate "$crate" --out "$out_dir" --test-threads 1
  cargo run -q -p xtask -- sdk coverage report \
    --scope "${crate}" \
    --summary "${out_dir}/coverage-summary.json" \
    --lcov "${out_dir}/coverage-lcov.info" \
    --out "${out_dir}/gate-report.json" \
    --fail-under-exec-lines 100 \
    --fail-under-functions 100 \
    --fail-under-branches 100 \
    --require-branches

  printf "%s\tpass\t100.0\t100.0\t100.0\t%s\n" "$crate" "${out_dir}/gate-report.json" >> target/coverage/coverage-refresh.tsv
  printf "%s\tpass\n" "$crate" >> target/coverage/coverage-refresh-status.tsv
done < "$required_file"

cargo run -q -p xtask -- sdk release preflight
echo "release preflight complete"
