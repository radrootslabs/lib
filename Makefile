.PHONY: all bindings clean help \
        bindings-tangle-schema bindings-types \
        build build-tangle-sql-wasm

SHELL := /bin/bash
.SHELLFLAGS := -e -o pipefail -c
TS_RS_FEATURE ?= ts-rs

BINDINGS_TARGETS := \
    bindings-tangle-schema \
    bindings-types

BUILD_TARGETS := \
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

bindings-tangle-schema:
	@(cd tangle-schema && cargo test --features $(TS_RS_FEATURE))
	@(cd tangle-schema/bindings/ts && yarn build)

bindings-types:
	@(cd types && cargo test --features $(TS_RS_FEATURE))
	@(cd types/bindings/ts && yarn build)

build-tangle-sql-wasm:
	wasm-pack build tangle-sql-wasm --release --target web --out-dir ../tangle-sql-wasm/pkg/dist --scope radroots
