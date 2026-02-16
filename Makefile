# variables
BINARY_NAME=kiroku
RELEASE_DIR=target/release
DIST_DIR=dist
VERSION=$(shell grep version Cargo.toml | head -n 1 | cut -d '"' -f 2)
# `grep version Cargo.toml`: all lines with the word `version`
# `head -n 1`: first found result
# `cut -d '"' -f 2`: split on " and take the second item

.PHONY: all build clean deb rpm tarball setup

all: build deb rpm tarball

setup:
		@echo "Installing packaging tools..."
		cargo install cargo-deb cargo-generate-rpm

build:
		@echo "Building release binary..."
		cargo build --release
		strip $RELEASE_DIR/$BINARY_NAME

clean:
		cargo clean
		rm -rf $(DIST_DIR)