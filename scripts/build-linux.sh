#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
if [[ -x "$root/.tools/cargo/bin/cargo" ]]; then
    export CARGO_HOME="$root/.tools/cargo" RUSTUP_HOME="$root/.tools/rustup"
    export PATH="$CARGO_HOME/bin:$PATH"
fi
if [[ -x "$root/.tools/go/bin/go" ]]; then
    export PATH="$root/.tools/go/bin:$PATH"
    export GOPATH="$root/.tools/gopath" GOCACHE="$root/.tools/go-cache"
fi
command -v cargo >/dev/null || { echo 'Rust is required; install the stable toolchain from https://rustup.rs.' >&2; exit 1; }
command -v go >/dev/null || { echo 'Go 1.27.3 or later is required to build the bundled VIIPER source.' >&2; exit 1; }
command -v gcc >/dev/null || { echo 'Install build-essential.' >&2; exit 1; }
(cd vendor/viiper-source && CGO_ENABLED=1 go build -buildmode=c-shared -trimpath -ldflags='-s -w' -o ../viiper/libVIIPER.so ./lib/viiper)
cargo build --release --locked
cp vendor/viiper/libVIIPER.so target/release/libVIIPER.so
mkdir -p target/release/ui
cp src/linux/graphics.py target/release/ui/graphics.py
cp assets/opends.png target/release/opends.png
cp scripts/setup-ubuntu.sh target/release/setup-ubuntu.sh
printf 'Built %s/target/release/opends\nRun ./opends to open the desktop window.\n' "$root"
