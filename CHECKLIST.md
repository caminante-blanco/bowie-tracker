# Final Draft Checklist

- [x] **Architecture & Analytics**
    - [x] Create `src/analytics.rs` for aggregating metrics.
    - [x] Rolling time windows (30d, 365d).
    - [x] Monthly Wrapped "Rewards" logic.

- [x] **Persistence Integration**
    - [x] IndexedDB (Rexie) for tracks and mappings.
    - [x] LocalStorage for user config.

- [ ] **Navigation & Pages**
    - [ ] State-based page routing (Main, Rewards, Charts, Settings).
    - [ ] **First-Time Setup Page**: Force setup if no username exists.
    - [ ] **Settings View**: Hide setup fields here after first run.

- [ ] **Data-Driven Visuals**
    - [ ] **Charts Page**: Implement SVG-based bar/line charts for listening history.
    - [ ] **Rewards View**: Refine "Wrapped" card layout.

- [x] **Final Polish**
    - [x] 12-hour time formatting.
    - [x] Projections (Day, Week, Month, Year).
    - [x] Auto-sync heartbeat.