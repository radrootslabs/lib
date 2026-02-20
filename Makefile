.PHONY: all build clean help export-ts-sdk-bindings \
	build-events-codec-wasm build-tangle-db-wasm build-tangle-events-wasm

SHELL := /bin/bash
.SHELLFLAGS := -e -o pipefail -c

BUILD_TARGETS := \
	build-events-codec-wasm \
	build-tangle-db-wasm \
	build-tangle-events-wasm

all: build

build: $(BUILD_TARGETS)

clean:
	cargo clean

help:
	@echo "Commands:"
	@echo "  make all"
	@echo "  make build"
	@echo "  make clean"
	@echo "  make export-ts-sdk-bindings"
	@echo "  make help"
	@printf "%s\n" $(BUILD_TARGETS)

export-ts-sdk-bindings:
	cargo run -q -p xtask -- sdk export-ts

build-tangle-db-wasm:
	wasm-pack build crates/tangle-db-wasm --release --target web \
		--out-dir ../tangle-db-wasm/pkg/dist --scope radroots

build-events-codec-wasm:
	wasm-pack build crates/events-codec-wasm --release --target web \
		--out-dir ../events-codec-wasm/pkg/dist --scope radroots

build-tangle-events-wasm:
	wasm-pack build crates/tangle-events-wasm --release --target web \
		--out-dir ../tangle-events-wasm/pkg/dist --scope radroots
