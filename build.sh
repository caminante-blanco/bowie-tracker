#!/bin/bash
set -e

# 1. Install Trunk binary (avoiding slow cargo install)
TRUNK_VERSION="v0.21.14"
# Force update to the newer version
curl -L "https://github.com/trunk-rs/trunk/releases/download/${TRUNK_VERSION}/trunk-x86_64-unknown-linux-gnu.tar.gz" | tar -xzf-

# 2. Build the project
./trunk build --release
