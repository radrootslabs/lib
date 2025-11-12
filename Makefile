.PHONY: all bindings bindings-tangle-schema bindings-types clean help
SHELL := /bin/bash
.SHELLFLAGS := -e -o pipefail -c
TS_RS_FEATURE ?= ts-rs

all: bindings

bindings: bindings-tangle-schema bindings-types
	@echo "All bindings built successfully."

bindings-tangle-schema:
	@echo "Building tangle-schema bindings"
	@(cd tangle-schema && cargo test --features $(TS_RS_FEATURE))
	@(cd tangle-schema/bindings/ts && yarn build)

bindings-types:
	@echo "Building types bindings"
	@(cd types && cargo test --features $(TS_RS_FEATURE))
	@(cd types/bindings/ts && yarn build)

clean:
	cargo clean

help:
	@echo "Usage:"
	@echo "  make bindings   Build all Rust + TS bindings for all crates"
	@echo "  make clean      Remove build artifacts"
	@echo "  make help       Show this help message"
