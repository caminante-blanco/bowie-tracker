#!/bin/bash
set -e

# 1. Install Trunk binary (avoiding slow cargo install)
TRUNK_VERSION="v0.21.4"
curl -L "https://github.com/trunk-rs/trunk/releases/download/${TRUNK_VERSION}/trunk-x86_64-unknown-linux-gnu.tar.gz" | tar -xzf-

# 2. Build the project
./trunk build --release

# 3. Manually copy the metadata into dist since Trunk only handles what is linked in index.html
cp bowie_metadata.json dist/
