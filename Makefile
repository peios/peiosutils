SHELL := bash
.SHELLFLAGS := -eu -o pipefail -c

ROOT  := $(CURDIR)
DIST  := $(ROOT)/dist
BUILD := $(ROOT)/build

# Use the rustup toolchain explicitly so we don't pick up a nix-shimmed rustc
# that can't find the musl rustlib.
RUSTUP_TC := $(HOME)/.rustup/toolchains/stable-x86_64-unknown-linux-gnu
CARGO     := $(RUSTUP_TC)/bin/cargo
RUSTC     := $(RUSTUP_TC)/bin/rustc
export CARGO RUSTC

PEIPKG_BUILD_SRC := $(abspath $(ROOT)/../peipkg-build)
PEIPKG_BUILD     := $(BUILD)/bin/peipkg-build
export PEIPKG_BUILD

TARGET := x86_64-unknown-linux-musl

# Auto-discover workspace members under tools/. Tool name = leaf directory.
TOOL_MANIFESTS := $(wildcard tools/*/*/Cargo.toml)
TOOLS          := $(notdir $(patsubst %/Cargo.toml,%,$(TOOL_MANIFESTS)))

# Single-package output: every workspace member's binary lands in one peipkg.
# Per-tool install_path metadata still controls layout inside the package.
BUNDLE_NAME    := peiosutils
BUNDLE_VERSION := 0.0.1
BUNDLE_PKG     := $(ROOT)/dist/$(BUNDLE_NAME)_$(BUNDLE_VERSION)-1_x86_64.peipkg

.PHONY: all pkg pkgs clean clean-all peipkg-build list $(TOOLS)

all: $(TOOLS)

# Build everything and produce the single peiosutils peipkg.
pkg pkgs: $(BUNDLE_PKG)

$(BUNDLE_PKG): $(TOOLS) $(PEIPKG_BUILD)
	@scripts/pack-bundle.sh

list:
	@printf '%s\n' $(TOOLS)

peipkg-build: $(PEIPKG_BUILD)

$(PEIPKG_BUILD):
	mkdir -p $(@D)
	cd $(PEIPKG_BUILD_SRC) && go build -o $(PEIPKG_BUILD) ./cmd/peipkg-build

# Per-tool build target (e.g. `make whoami-token` rebuilds just that one).
# Cargo is incremental so re-running is cheap.
$(TOOLS):
	@scripts/build-tool.sh $@

clean:
	rm -rf $(BUILD) $(DIST)
	mkdir -p $(DIST)
	touch $(DIST)/.gitkeep

clean-all: clean
	$(CARGO) clean
