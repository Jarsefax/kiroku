#!/bin/sh
set -eu

usage() {
    printf 'Usage: ./install.sh [--docker|--podman]\n\nInstalls krk into ~/.local/bin.\nDefault: use local Rust (1.88 or later), then Docker, then Podman.\n--docker: build in Docker instead of using local Rust.\n--podman: build in Podman instead of using local Rust.\n'
}

mode=auto
case "${1:-}" in
    --docker) mode=docker ;;
    --podman) mode=podman ;;
    --help|-h) usage; exit 0 ;;
    '') ;;
    *) usage >&2; exit 1 ;;
esac
if [ "$#" -gt 1 ]; then
    usage >&2
    exit 1
fi

repo_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
bin_dir="$HOME/.local/bin"

if [ "$mode" = auto ]; then
    # Version checks also detect rustup shims without an installed toolchain.
    if command -v cargo >/dev/null 2>&1 && command -v rustc >/dev/null 2>&1 &&
        cargo --version >/dev/null 2>&1 && rustc --version >/dev/null 2>&1; then
        mode=local
    elif command -v docker >/dev/null 2>&1; then
        mode=docker
    elif command -v podman >/dev/null 2>&1; then
        mode=podman
    else
        printf 'No Rust toolchain, Docker, or Podman found. Install Rust 1.88 or later, Docker, or Podman, then retry ./install.sh.\n' >&2
        exit 1
    fi
    printf 'Building with %s.\n' "$mode"
fi

if [ "$mode" != local ]; then
    if [ "$(uname -s)" != Linux ]; then
        printf 'The container installer currently supports Linux hosts only.\n' >&2
        exit 1
    fi
    case "$(uname -m)" in
        x86_64) platform=linux/amd64 ;;
        aarch64|arm64) platform=linux/arm64 ;;
        *) printf 'The container installer supports x86-64 and ARM64 only.\n' >&2; exit 1 ;;
    esac
    if ! command -v "$mode" >/dev/null 2>&1; then
        printf '%s is missing. Install it, then retry ./install.sh --%s.\n' "$mode" "$mode" >&2
        exit 1
    fi
    "$mode" info >/dev/null
    image=docker.io/library/rust:1.88-alpine
    "$mode" pull --platform "$platform" "$image"
    container_id=
    staging_dir=$(mktemp -d)
    cleanup() {
        if [ -n "$container_id" ]; then
            "$mode" rm -f "$container_id" >/dev/null 2>&1 || :
        fi
        rm -rf "$staging_dir"
    }
    trap cleanup EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
    # Copy only build inputs; no host mounts or root-owned checkout files.
    container_id=$("$mode" create --platform "$platform" --workdir /build "$image" \
        sh -c 'cargo build --release --locked --target "$(rustc -vV | sed -n "s/^host: //p")" && cp target/*/release/krk /tmp/krk')
    "$mode" cp "$repo_dir/Cargo.toml" "$container_id:/build/Cargo.toml"
    "$mode" cp "$repo_dir/Cargo.lock" "$container_id:/build/Cargo.lock"
    "$mode" cp "$repo_dir/src" "$container_id:/build/src"
    "$mode" cp "$repo_dir/.cargo" "$container_id:/build/.cargo"
    "$mode" start --attach "$container_id"
    if [ "$("$mode" inspect --format '{{.State.ExitCode}}' "$container_id")" != 0 ]; then
        printf 'Container build failed; krk was not installed.\n' >&2
        exit 1
    fi
    "$mode" cp "$container_id:/tmp/krk" "$staging_dir/krk"
    "$staging_dir/krk" --version
    install -Dm755 "$staging_dir/krk" "$bin_dir/krk"
else
    # Cargo handles locating the built binary, including custom target settings.
    cargo install --path "$repo_dir" --locked --force --root "$HOME/.local"
fi

printf '\nInstalled krk into %s.\n' "$bin_dir"
case ":${PATH:-}:" in
    *":$bin_dir:"*) printf 'Run krk to get started.\n' ;;
    *)
        printf 'Add ~/.local/bin to your shell PATH to run krk by name.\n'
        printf 'For this session, run: export PATH="$HOME/.local/bin:$PATH"\n'
        printf 'You can also run it directly: "%s/krk"\n' "$bin_dir"
        ;;
esac
