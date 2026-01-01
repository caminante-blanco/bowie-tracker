# Ziggy Integration Guide: Spotify & Last.fm

This document outlines the technical requirements and architectural path for expanding Ziggy's data sources beyond ListenBrainz.

## 1. Spotify Integration (Instant Sync)

To provide "Now Playing" and "Recent History" without a backend, Ziggy must use the **OAuth2 PKCE (Proof Key for Code Exchange)** flow.

### Setup Requirements
1.  **Developer Portal:** Register an application at [developer.spotify.com](https://developer.spotify.com/dashboard).
2.  **Redirect URI:** Set to your production URL (e.g., `https://ziggy.pages.dev/`).
3.  **Scopes Required:** `user-read-currently-playing`, `user-read-recently-played`.

### Technical Implementation
*   **Flow:**
    1. Generate a cryptographically strong `code_verifier`.
    2. Generate a `code_challenge` (SHA256 hash of the verifier).
    3. Redirect user to `https://accounts.spotify.com/authorize`.
    4. Handle the redirect back to Ziggy, capturing the `code` parameter.
    5. Exchange `code` + `code_verifier` for an `access_token` via a POST request.
*   **Rust Resources:**
    *   [rspotify](https://docs.rs/rspotify/latest/rspotify/) crate with `client-browser` and `pkce` features.
*   **Constraint:** The API only provides the **last 50 tracks**. The app must save these to IndexedDB upon every visit to build a persistent history.

---

## 2. Spotify Historical Data (Deep Sync)

For users who want their entire lifetime of Bowie stats, implement a local JSON parser for Spotify "Extended Streaming History" exports.

### User Flow
1. User requests "Extended Streaming History" from Spotify Privacy settings.
2. User receives a ZIP file (usually after 1-5 days).
3. User drags `endsong_x.json` files into Ziggy.

### Technical Implementation
*   **Parsing:** Use `serde_json` to parse files in the browser. Since it is WASM, processing 100MB+ files is highly efficient.
*   **Deduplication:** Spotify uses a unique `spotify_track_uri` and a high-precision `ts` (timestamp). Use `ts + track_uri` as a unique identifier in IndexedDB to prevent duplicates.
*   **MBID Mapping:** Spotify does not provide MusicBrainz IDs. Use the title-based lookup map in Ziggy's `bowie_metadata.json` to link Spotify listens to canonical MusicBrainz data.

---

## 3. Last.fm Integration

Last.fm is the easiest "mainstream" source to implement as it requires no complex OAuth for read-only history.

### Implementation
*   **Authentication:** Requires a standard API Key (passed as a URL parameter).
*   **Endpoint:** `user.getRecentTracks`.
*   **Benefit:** Last.fm often provides **MusicBrainz IDs (MBIDs)** directly in the response, making integration with Ziggy's charts near-perfect.
*   **Resources:** [Last.fm API Documentation](https://www.last.fm/api/show/user.getRecentTracks).

---

## 4. Architecture: The "Hybrid" Model

To minimize user friction and maximize data accuracy, Ziggy should follow this hierarchy:

1.  **The Seed:** Use **Spotify JSON Upload** or **Last.fm History Sync** to populate the local database with years of Bowie history.
2.  **The Patch:** Use **Spotify API (Last 50)** to automatically fill in the gaps since the last visit.
3.  **The Source:** Add a `source` field to the `Listen` model in `src/models.rs` to track the origin of each scrobble (ListenBrainz, SpotifyAPI, SpotifyFile, Lastfm).

## 5. Storage (IndexedDB)

The `rexie` crate handles the local storage. When merging multiple sources:
*   Standardize all incoming data into the `Listen` struct.
*   Use a composite key or a unique index on `(listened_at, track_name)` to ensure that if a user has both Spotify and Last.fm linked, the same listen isn't counted twice.
