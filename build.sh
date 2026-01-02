#!/bin/bash
set -e

# Ensure .cargo/bin is in PATH for both the script and sub-processes
export PATH="$HOME/.cargo/bin:$PATH"

# Ensure WASM target is installed
if command -v rustup &> /dev/null; then
    rustup target add wasm32-unknown-unknown
fi

# Install Trunk if missing
TRUNK_VERSION="v0.21.14"
if [ ! -f ./trunk ]; then
    curl -L "https://github.com/trunk-rs/trunk/releases/download/${TRUNK_VERSION}/trunk-x86_64-unknown-linux-gnu.tar.gz" | tar -xzf-
fi

# Build project with Trunk
./trunk build --release
