#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out_dir="${1:-${repo_root}/target/ts-rs}"

run_export() {
  local crate="$1"
  local sdk_package="$2"
  RADROOTS_TS_RS_EXPORT_DIR="${out_dir}/${sdk_package}" \
    cargo test -q -p "${crate}" --features ts-rs
}

rm -rf "${out_dir}"
mkdir -p "${out_dir}"

cd "${repo_root}"

run_export "radroots-events" "events"
run_export "radroots-trade" "trade"
run_export "radroots-types" "types"
run_export "radroots-tangle-db-schema" "tangle-db-schema"
run_export "radroots-identity" "identity"

find "${out_dir}" -maxdepth 2 -type f | sort
