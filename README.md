# Bowie Tracker 👨‍🎤

A high-performance Rust/WASM web application built with the **Leptos** framework to track David Bowie listening statistics via **ListenBrainz**.

## Features

- **Real-time Sync:** Connects to ListenBrainz to fetch recent listening history.
- **Fast Normalization:** Uses a custom IndexedDB-backed mapping system to handle Messy IDs and MBIDs.
- **Local-First:** All statistics and metadata are stored and processed in your browser's IndexedDB.
- **Gruvbox Theme:** Styled with a clean, dark-mode aesthetic.

## Development

This project uses **Trunk** for building and bundling.

```bash
# Install Trunk
cargo install --locked trunk

# Build and serve locally
trunk serve
```

## Deployment

The project is automatically deployed to **Cloudflare Pages** via GitHub Actions on every push to the `main` branch.
