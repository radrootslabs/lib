.PHONY: all bindings clean help

SHELL := /bin/bash
.SHELLFLAGS := -e -o pipefail -c

all: bindings

bindings:
	cd tangle-schema && cargo test && cd bindings/ts && yarn build
	@echo "Building tangle-sql-core bindings"
	cd tangle-sql-core && cargo test && cd bindings/ts && yarn build
	@echo "Building types bindings"
	cd types && cargo test && cd bindings/ts && yarn build
	@echo "All bindings built successfully."

clean:
	cargo clean

help:
	@echo "Usage:"
	@echo "  make bindings   Build all Rust + TS bindings for all crates"
	@echo "  make clean      Remove build artifacts"
	@echo "  make help       Show this help message"
