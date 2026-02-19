.PHONY: all build clean help \
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
	@echo "  make help"
	@printf "%s\n" $(BUILD_TARGETS)

build-tangle-db-wasm:
	wasm-pack build tangle-db-wasm --release --target web \
		--out-dir ../tangle-db-wasm/pkg/dist --scope radroots

build-events-codec-wasm:
	wasm-pack build events-codec-wasm --release --target web \
		--out-dir ../events-codec-wasm/pkg/dist --scope radroots

build-tangle-events-wasm:
	wasm-pack build tangle-events-wasm --release --target web \
		--out-dir ../tangle-events-wasm/pkg/dist --scope radroots
