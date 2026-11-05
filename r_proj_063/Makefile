# Builds the tokio demo programs in src/examples.
#
#   make                 interactive menu: pick the demo to build
#   make tokio_io        build one demo
#   make run-tokio_io    build and run one demo
#   make all             build all demos
#   make list            show the available demos
#   make clean           remove the build output
#
# Add RELEASE=1 to any build or run target for an optimized build.
#
# Each file in src/examples also needs an [[example]] entry in Cargo.toml,
# because Cargo only finds examples on its own in the top-level examples/ folder.
# Add RELEASE=1 to any of the build for optimized code

SHELL := /bin/bash

EXAMPLES := $(sort $(basename $(notdir $(wildcard src/examples/*.rs))))
RUN_TARGETS := $(addprefix run-,$(EXAMPLES))

ifdef RELEASE
  CARGO_FLAGS := --release
  PROFILE := release
else
  CARGO_FLAGS :=
  PROFILE := debug
endif

.DEFAULT_GOAL := menu
.PHONY: menu all list clean $(EXAMPLES) $(RUN_TARGETS)

menu:
	@PS3="Which demo do you want to build? "; \
	select choice in $(EXAMPLES) all quit; do \
		case "$$choice" in \
			"") echo "Please enter one of the listed numbers." ;; \
			quit) exit 0 ;; \
			*) exec $(MAKE) --no-print-directory "$$choice" ;; \
		esac; \
	done

$(EXAMPLES):
	cargo build $(CARGO_FLAGS) --example $@
	@echo "Built: target/$(PROFILE)/examples/$@"

$(RUN_TARGETS): run-%:
	cargo run $(CARGO_FLAGS) --example $*

all:
	cargo build $(CARGO_FLAGS) --examples
	@echo "Built in target/$(PROFILE)/examples: $(EXAMPLES)"

list:
	@printf '%s\n' $(EXAMPLES)

clean:
	cargo clean
