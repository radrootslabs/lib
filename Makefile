.PHONY: all bindings clean help \
        bindings-events bindings-tangle-schema bindings-trade bindings-types \
        build build-events-codec-wasm build-tangle-sql-wasm

SHELL := /bin/bash
.SHELLFLAGS := -e -o pipefail -c
TS_RS_FEATURE ?= ts-rs

BINDINGS_TARGETS := \
    bindings-events \
    bindings-tangle-schema \
    bindings-trade \
    bindings-types

BUILD_TARGETS := \
    build-events-codec-wasm \
    build-tangle-sql-wasm

all: bindings build

bindings: $(BINDINGS_TARGETS)

build: $(BUILD_TARGETS)

clean:
	cargo clean

help:
	@echo "Commands:"
	@echo "  make all"
	@echo "  make bindings"
	@echo "  make build"
	@echo "  make clean"
	@echo "  make help"
	@printf "%s\n" $(BINDINGS_TARGETS)
	@printf "%s\n" $(BUILD_TARGETS)

bindings-events:
	@(cd events && cargo test --features $(TS_RS_FEATURE))
	@(cd events/bindings/ts && npm run build)

bindings-tangle-schema:
	@(cd tangle-schema && cargo test --features $(TS_RS_FEATURE))
	@(cd tangle-schema/bindings/ts && npm run build)

bindings-trade:
	@(cd trade && cargo test --features $(TS_RS_FEATURE))
	@(cd trade/bindings/ts && npm run build)

bindings-types:
	@(cd types && cargo test --features $(TS_RS_FEATURE))
	@(cd types/bindings/ts && npm run build)

build-tangle-sql-wasm:
	wasm-pack build tangle-sql-wasm --release --target web \
		--out-dir ../tangle-sql-wasm/pkg/dist --scope radroots

build-events-codec-wasm:
	wasm-pack build events-codec-wasm --release --target web \
		--out-dir ../events-codec-wasm/pkg/dist --scope radroots
