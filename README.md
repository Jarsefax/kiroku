# TODO
## look into
- directories
- anyhow
- serde
- ctrlc

# LLM suggestions
## crates
1. clap - argument parsing
2. directories - storage locations
3. anyhow - stderr output
4. serde - saving/loading
5. ctrlc - abort handling

## tools
1. help2man eller pandoc - .md till man page
2. cargo-deb - create Debian package
3. cargo-generate-rpm - create Red Hat package
4. make - för att skapa de olika distributions-paketen

## Makefile
A Makefile is the classic way to orchestrate these tasks. It lets you automate the "manual" steps into a single command, ensuring you don't forget to strip the binary or update the version.

Here is a professional-grade Makefile for a Rust CLI tool.

### The Makefile
Create a file named Makefile (no extension) in your project root:
```
Makefile
# Variables
BINARY_NAME=my_tool
VERSION=$(shell grep version Cargo.toml | head -n 1 | cut -d '"' -f 2)
RELEASE_DIR=target/release
DIST_DIR=dist

.PHONY: all build clean deb rpm tarball setup

all: build deb rpm tarball

setup:
@echo "Installing packaging tools..."
cargo install cargo-deb cargo-generate-rpm

build:
@echo "Building release binary..."
cargo build --release
strip $(RELEASE_DIR)/$(BINARY_NAME)

deb: build
@echo "Creating Debian package..."
cargo deb

rpm: build
@echo "Creating RPM package..."
cargo generate-rpm

tarball: build
@echo "Creating generic Linux tarball..."
mkdir -p $(DIST_DIR)
tar -czvf $(DIST_DIR)/$(BINARY_NAME)-v$(VERSION)-linux-x86_64.tar.gz -C $(RELEASE_DIR) $(BINARY_NAME)

clean:
cargo clean
rm -rf $(DIST_DIR)
```

### How to use it
- To build everything: Just type make. It will compile, strip, and package your app into .deb, .rpm, and .tar.gz formats.
- To build just one: Run make deb or make tarball.
- To clean up: Run make clean to wipe the build artifacts and start fresh.

#### Why these steps matter
- strip: By default, Rust binaries include a lot of "debug symbols." Running strip can reduce your binary size from 20MB down to 2MB. This is standard practice for distributed Linux software.
- .PHONY: This tells make that build, clean, etc., are names of commands, not actual files in your directory. 
- The Version Hack: The line $(shell grep version...) automatically pulls the version number from your Cargo.toml. This prevents "version mismatch" where your installer says 1.0.0 but the binary says 0.9.0.

### Testing your "Manual" work
Before you send these files to users, you should verify they follow the standards we talked about earlier:
- Check paths: Run dpkg -c target/debian/*.deb. You should see your binary sitting in /usr/bin/.
- Check behavior: Install the package and run it. Does it create its config in ~/.config/my_tool?
- Check help: Run my_tool --help. It should look clean and provide all the info a user needs.
- 