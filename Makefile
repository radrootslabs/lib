.PHONY: all build clean help \
	clean-hyphenated-wasm-output \
	build-events-codec-wasm build-replica-db-wasm build-replica-sync-wasm

SHELL := /bin/bash
.SHELLFLAGS := -e -o pipefail -c

EVENTS_CODEC_WASM_PACKAGE := @radroots/radroots_events-codec-wasm
EVENTS_CODEC_WASM_DESCRIPTION := WebAssembly bindings for radroots_events_codec
REPLICA_DB_WASM_PACKAGE := @radroots/radroots_replica-db-wasm
REPLICA_DB_WASM_DESCRIPTION := WebAssembly bindings for radroots_replica_db
REPLICA_SYNC_WASM_PACKAGE := @radroots/radroots_replica-sync-wasm
REPLICA_SYNC_WASM_DESCRIPTION := WebAssembly bindings for radroots_replica_sync

HYPHENATED_WASM_OUTPUT_DIRS := \
	crates/events-codec-wasm \
	crates/replica-db-wasm \
	crates/replica-sync-wasm

BUILD_TARGETS := \
	build-events-codec-wasm \
	build-replica-db-wasm \
	build-replica-sync-wasm

all: build

build: clean-hyphenated-wasm-output $(BUILD_TARGETS)

clean:
	cargo clean

help:
	@echo "Commands:"
	@echo "  make all"
	@echo "  make build"
	@echo "  make clean"
	@echo "  make help"
	@printf "%s\n" $(BUILD_TARGETS)

clean-hyphenated-wasm-output:
	rm -rf $(HYPHENATED_WASM_OUTPUT_DIRS)

normalize_wasm_package_json = python3 -c 'import json, pathlib, sys; path = pathlib.Path(sys.argv[1]); data = json.loads(path.read_text()); data["name"] = sys.argv[2]; data["description"] = sys.argv[3]; path.write_text(json.dumps(data, indent=2) + "\n")' "$(1)" "$(2)" "$(3)"

build-replica-db-wasm:
	wasm-pack build crates/replica_db_wasm --release --target web \
		--out-dir pkg/dist --scope radroots
	$(call normalize_wasm_package_json,crates/replica_db_wasm/pkg/dist/package.json,$(REPLICA_DB_WASM_PACKAGE),$(REPLICA_DB_WASM_DESCRIPTION))

build-events-codec-wasm:
	wasm-pack build crates/events_codec_wasm --release --target web \
		--out-dir pkg/dist --scope radroots
	$(call normalize_wasm_package_json,crates/events_codec_wasm/pkg/dist/package.json,$(EVENTS_CODEC_WASM_PACKAGE),$(EVENTS_CODEC_WASM_DESCRIPTION))

build-replica-sync-wasm:
	wasm-pack build crates/replica_sync_wasm --release --target web \
		--out-dir pkg/dist --scope radroots
	$(call normalize_wasm_package_json,crates/replica_sync_wasm/pkg/dist/package.json,$(REPLICA_SYNC_WASM_PACKAGE),$(REPLICA_SYNC_WASM_DESCRIPTION))
