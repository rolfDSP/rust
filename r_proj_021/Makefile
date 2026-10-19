SHELL := /bin/bash

.PHONY: help

help: ## Show all commands with their descriptions
	@grep -E '^[a-zA-Z0-9_-]+:.*## ' $(MAKEFILE_LIST) | sed 's/\r$$//' | awk 'BEGIN {FS = ":.*## "}; {printf "\033[36m%-15s\033[0m %s\n", $$1, $$2}'

clean: ## Clean the project
	cargo clean

build: ## build the project
	cargo build

lint:
	cargo clippy

format:
	cargo fmt

