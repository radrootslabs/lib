.PHONY: all build clean help export-ts-sdk-bindings \
	build-events-codec-wasm build-replica-db-wasm build-replica-sync-wasm

SHELL := /bin/bash
.SHELLFLAGS := -e -o pipefail -c

BUILD_TARGETS := \
	build-events-codec-wasm \
	build-replica-db-wasm \
	build-replica-sync-wasm

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

build-replica-db-wasm:
	wasm-pack build crates/replica-db-wasm --release --target web \
		--out-dir ../replica-db-wasm/pkg/dist --scope radroots

build-events-codec-wasm:
	wasm-pack build crates/events-codec-wasm --release --target web \
		--out-dir ../events-codec-wasm/pkg/dist --scope radroots

build-replica-sync-wasm:
	wasm-pack build crates/replica-sync-wasm --release --target web \
		--out-dir ../replica-sync-wasm/pkg/dist --scope radroots
