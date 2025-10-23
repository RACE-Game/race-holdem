.PHONY: release-all build-race-omaha-cash

CRATES = race-omaha-cash race-holdem-cash race-holdem-mtt-table race-holdem-mtt

all: $(CRATES)

define BUILD_template
.PHONY: $(1)
$(1):
	cargo build -r --target wasm32-unknown-unknown -p $(1)
	wasm-opt -Oz target/wasm32-unknown-unknown/release/$$(subst -,_,$(1)).wasm -o target/$$(subst -,_,$(1)).wasm
endef

$(foreach crate,$(CRATES), $(eval $(call BUILD_template,$(crate))))
