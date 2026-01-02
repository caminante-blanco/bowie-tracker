use leptos::*;
use chrono::Utc;
use std::collections::HashMap;

use bowie_tracker::models::{Listen, ListenBrainzResponse, PlayingNowResponse, BowieLookup, PlayingNowListen};
use bowie_tracker::analytics::{calculate_metrics, format_relative_time, MonthlyWrapped, DayStats, match_playing_now};
use bowie_tracker::db::{init_db, add_listens, get_all_listens, get_max_timestamp, get_all_album_metadata};
use bowie_tracker::charts::ListeningHistoryChart;
use bowie_tracker::api::fetch_with_rate_limit;

#[derive(Clone, Copy, PartialEq)]
enum Page { Dashboard, Rewards, Charts, Settings, DayDetail }

fn main() {
    console_error_panic_hook::set_once();
    web_sys::console::log_1(&"Ziggy Tracker v2026.01.02.2 - High Frequency Sync".into());
    mount_to_body(|| view! { <App/> })
}

#[component]
fn App() -> impl IntoView {
    let (listens, set_listens) = create_signal(Vec::<Listen>::new());
    let (now_playing, set_now_playing) = create_signal(None::<PlayingNowListen>);
    let (username, set_username) = create_signal(String::new());
    let (token, set_token) = create_signal(String::new());
    let (is_syncing, set_is_syncing) = create_signal(false);
    let (status, set_status) = create_signal("Ready".to_string());
    
    let (track_counts, set_track_counts) = create_signal(HashMap::<String, usize>::new());
    let (bowie_lookup, set_bowie_lookup) = create_signal(None::<BowieLookup>);

    let (playback_start, set_playback_start) = create_signal(None::<i64>);
    let (now_ts, set_now_ts) = create_signal(Utc::now().timestamp());

    let (last_np_json, set_last_np_json) = create_signal(String::new());

    create_effect(move |_| {
        let handle = gloo_timers::callback::Interval::new(1_000, move || {
            set_now_ts.set(Utc::now().timestamp());
        });
        move || drop(handle)
    });

    create_effect(move |_| {
        spawn_local(async move {
            let win = web_sys::window().unwrap();
            let origin = win.location().origin().unwrap_or_default();
            let url = format!("{}/bowie_lookup.json", origin);
            match reqwest::get(&url).await {
                Ok(resp) => {
                    if let Ok(db) = resp.json::<BowieLookup>().await {
                        set_bowie_lookup.set(Some(db));
                    }
                },
                Err(e) => web_sys::console::log_1(&format!("Fetch error: {}", e).into()),
            }
        });
    });

    let (current_page, set_current_page) = create_signal(Page::Dashboard);
    let (proj_basis, set_proj_basis) = create_signal("DAY".to_string());
    let (is_setup, set_is_setup) = create_signal(false); 
    let (display_count, _set_display_count) = create_signal(50);

    let (selected_month, set_selected_month) = create_signal(None::<MonthlyWrapped>);
    let (selected_day, set_selected_day) = create_signal(None::<DayStats>);

    let metrics = create_memo(move |_| {
        if let Some(lookup) = bowie_lookup.get() {
            calculate_metrics(&listens.get(), Utc::now(), &proj_basis.get(), &track_counts.get(), &lookup)
        } else {
            Default::default()
        }
    });

    let refresh_data = move || {
        spawn_local(async move {
            if let Ok(db) = init_db().await {
                if let Ok(mut all) = get_all_listens(&db).await {
                    all.sort_by_key(|l| std::cmp::Reverse(l.listened_at));
                    set_listens.set(all);
                }
                if let Ok(counts) = get_all_album_metadata(&db).await {
                    set_track_counts.set(counts);
                }
            }
        });
    };

    let sync_history = move || {
        let user = username.get_untracked();
        let user_token = token.get_untracked();
        let lookup_opt = bowie_lookup.get_untracked();
        if user.is_empty() || is_syncing.get_untracked() || lookup_opt.is_none() { return; }
        
        spawn_local(async move {
            set_is_syncing.set(true);
            if let Ok(db) = init_db().await {
                let latest_ts = get_max_timestamp(&db).await.unwrap_or(None).unwrap_or(0);
                let url = format!("https://api.listenbrainz.org/1/user/{}/listens?count=1000&min_ts={}", user, latest_ts + 1);
                if let Ok(resp) = fetch_with_rate_limit(&url, &user_token).await {
                    if let Ok(json) = resp.json::<ListenBrainzResponse>().await {
                        let batch = json.payload.listens;
                        if !batch.is_empty() {
                            let _ = add_listens(&db, batch).await;
                            refresh_data();
                        }
                    }
                }
            }
            set_is_syncing.set(false);
        });
    };

    let sync_now_playing = move || {
        let user = username.get_untracked();
        let user_token = token.get_untracked();
        let lookup_opt = bowie_lookup.get_untracked();
        if user.is_empty() || lookup_opt.is_none() { return; }

        spawn_local(async move {
            let url = format!("https://api.listenbrainz.org/1/user/{}/playing-now", user);
            if let Ok(resp) = fetch_with_rate_limit(&url, &user_token).await {
                if let Ok(text) = resp.text().await {
                    // Optimization: Only process if the JSON content changed
                    if text == last_np_json.get_untracked() { return; }
                    set_last_np_json.set(text.clone());

                    if let Ok(json) = serde_json::from_str::<PlayingNowResponse>(&text) {
                        if let Some(lookup) = &lookup_opt {
                            // Get hint from last listen
                            let first_listen = listens.get_untracked().first().cloned();
                            web_sys::console::log_1(&format!("DEBUG: History Tip: {:?}", first_listen.as_ref().map(|l| &l.track_metadata.track_name)).into());
                            
                            let last_rg_id_string = first_listen.as_ref().and_then(|l| {
                                match_playing_now(&l.track_metadata, lookup, None).map(|(_rec_id, rg_id)| rg_id)
                            });
                            let last_rg_id = last_rg_id_string.as_ref();
                            
                            web_sys::console::log_1(&format!("DEBUG: Last RG ID Hint: {:?}", last_rg_id).into());

                            let matches: Vec<_> = json.payload.listens.into_iter().filter(|l| {
                                let is_match = match_playing_now(&l.track_metadata, lookup, last_rg_id).is_some();
                                web_sys::console::log_1(&format!("DEBUG: Checking '{}': Match? {}", l.track_metadata.track_name, is_match).into());
                                is_match
                            }).collect();

                            let new_np = matches.first().cloned();
                            web_sys::console::log_1(&format!("DEBUG: New NP found: {:?}", new_np.as_ref().map(|n| &n.track_metadata.track_name)).into());
                            let current_np = now_playing.get_untracked();
                            
                            let new_id = new_np.as_ref().map(|l| (l.track_metadata.track_name.clone(), l.track_metadata.artist_name.clone()));
                            let old_id = current_np.as_ref().map(|l| (l.track_metadata.track_name.clone(), l.track_metadata.artist_name.clone()));

                            if let Some(nid) = &new_id {
                                let mut start_ts = playback_start.get_untracked();
                                
                                if start_ts.is_none() || new_id != old_id {
                                    // 1. Try to restore from local storage
                                    let mut restored = false;
                                    if let Some(win) = web_sys::window() {
                                        if let Ok(Some(s)) = win.local_storage() {
                                            if let (Ok(Some(saved_id_str)), Ok(Some(saved_ts_str))) = (s.get_item("np_id"), s.get_item("np_start_ts")) {
                                                if let Ok(saved_id) = serde_json::from_str::<(String, String)>(&saved_id_str) {
                                                    if &saved_id == nid {
                                                        if let Ok(ts) = saved_ts_str.parse::<i64>() {
                                                            start_ts = Some(ts);
                                                            restored = true;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // 2. Try to cross-reference with history for perfect start time
                                    if let Some(l) = listens.get_untracked().first() {
                                        // Match on name and artist
                                        if l.track_metadata.track_name == nid.0 && l.track_metadata.artist_name == nid.1 {
                                            start_ts = Some(l.listened_at);
                                            restored = true;
                                        }
                                    }
                                    
                                    if !restored {
                                        let now = Utc::now().timestamp();
                                        start_ts = Some(now);
                                        if let Some(win) = web_sys::window() {
                                            if let Ok(Some(s)) = win.local_storage() {
                                                let _ = s.set_item("np_start_ts", &now.to_string());
                                                let _ = s.set_item("np_id", &serde_json::to_string(nid).unwrap_or_default());
                                            }
                                        }
                                    }
                                    set_playback_start.set(start_ts);
                                } else {
                                    // Even if track didn't change, history might have appeared now
                                    if let Some(l) = listens.get_untracked().first() {
                                        if l.track_metadata.track_name == nid.0 && l.track_metadata.artist_name == nid.1 {
                                            if start_ts != Some(l.listened_at) {
                                                set_playback_start.set(Some(l.listened_at));
                                            }
                                        }
                                    }
                                }
                            } else {
                                set_playback_start.set(None);
                                if let Some(win) = web_sys::window() {
                                    if let Ok(Some(s)) = win.local_storage() {
                                        let _ = s.remove_item("np_start_ts");
                                        let _ = s.remove_item("np_id");
                                    }
                                }
                            }
                            set_now_playing.set(new_np);
                        }
                    }
                }
            }
        });
    };

    create_effect(move |_| {
        if bowie_lookup.get().is_some() && is_setup.get() {
            sync_history();
            sync_now_playing();
        }
    });

    create_effect(move |_| {
        let h_handle = gloo_timers::callback::Interval::new(30_000, move || sync_history());
        let np_handle = gloo_timers::callback::Interval::new(1_000, move || sync_now_playing());
        move || { drop(h_handle); drop(np_handle); }
    });

    create_effect(move |_| {
        if let Some(win) = web_sys::window() {
            if let Ok(Some(storage)) = win.local_storage() {
                let u = storage.get_item("lb_username").unwrap_or_default().unwrap_or_default();
                let t = storage.get_item("lb_token").unwrap_or_default().unwrap_or_default();
                set_username.set(u.clone()); set_token.set(t);
                set_is_setup.set(!u.is_empty());
            }
        }
        refresh_data();
    });

    let sync_action = create_action(move |_: &()| async move { sync_history(); sync_now_playing(); });

    let deep_sync = create_action(move |_: &()| {
        let user = username.get();
        let user_token = token.get();
        async move {
            if user.is_empty() { return; }
            set_is_syncing.set(true);
            if let Ok(db) = init_db().await {
                let mut oldest_ts = Utc::now().timestamp();
                for i in 0..20 {
                    set_status.set(format!("DEEP SYNC {}/20", i+1));
                    let url = format!("https://api.listenbrainz.org/1/user/{}/listens?count=1000&max_ts={}", user, oldest_ts - 1);
                    if let Ok(resp) = fetch_with_rate_limit(&url, &user_token).await {
                        if let Ok(json) = resp.json::<ListenBrainzResponse>().await {
                            let batch = json.payload.listens;
                            if batch.is_empty() { break; }
                            oldest_ts = batch.iter().map(|l| l.listened_at).min().unwrap_or(oldest_ts);
                            let _ = add_listens(&db, batch).await;
                            refresh_data();
                        } else { break; }
                    } else { break; }
                }
                set_status.set("HISTORY READY".to_string());
            }
            set_is_syncing.set(false);
            set_status.set("Ready".to_string());
        }
    });

    let format_mins = |mins: i64| if mins >= 60 { format!("{:.1}h", mins as f64 / 60.0) } else { format!("{}m", mins) };

    view! {
        <div class="app-container" style="max-width: 900px; margin: 0 auto; padding: 10px;">
            {move || if !is_setup.get() { view! {
                <div class="card setup-overlay" style="position: fixed; inset: 0; z-index: 100; background: var(--bg-color); display: flex; flex-direction: column; align-items: center; justify-content: center; padding: 30px; text-align: center;">
                    <h1 style="color: var(--primary); font-size: 5rem; margin-bottom: 0;">"Ziggy"</h1>
                    <p style="color: #a89984; font-size: 1.2rem; margin-bottom: 40px; font-style: italic; max-width: 450px; line-height: 1.4;">
                        "if you stay with us, you're gonna be pretty Kookie too..."
                    </p>
                    <div style="width: 100%; max-width: 350px; display: flex; flex-direction: column; gap: 15px;">
                        <label for="username_setup" class="visually-hidden">"ListenBrainz Username"</label>
                        <input id="username_setup" type="text" placeholder="ListenBrainz Username" on:input=move |ev| { let v = event_target_value(&ev); set_username.set(v.clone()); if let Some(win) = web_sys::window() { if let Ok(Some(s)) = win.local_storage() { let _ = s.set_item("lb_username", &v); } } } style="padding: 15px; font-size: 1.1rem;"/>
                        <button on:click=move |_| { sync_history(); sync_now_playing(); deep_sync.dispatch(()); set_is_setup.set(true); } style="padding: 20px; background: var(--primary); font-weight: bold; letter-spacing: 2px;">"INITIALIZE"</button>
                    </div>
                </div>
            }.into_view() } else { view! { <div></div> }.into_view() }}

            <header style="background: var(--card-bg); padding: 12px; border-radius: 12px; margin-bottom: 15px; position: sticky; top: 10px; z-index: 50; display: flex; justify-content: space-between; align-items: center; box-shadow: 0 4px 15px rgba(0,0,0,0.4);">
                <div style="display: flex; align-items: center; gap: 15px;">
                    <h1 style="color: var(--primary); margin: 0; font-size: 1.4rem; font-weight: 900; letter-spacing: 3px;">"ZIGGY"</h1>
                    <span style="font-size: 0.6rem; color: var(--accent); font-weight: 900; letter-spacing: 1px; background: var(--surface); padding: 2px 8px; border-radius: 4px;">{move || status.get().to_uppercase()}</span>
                </div>
                <nav style="display: flex; gap: 15px; font-size: 0.8rem; font-weight: bold; letter-spacing: 1px;" role="navigation" aria-label="Main menu">
                    <button class="nav-link" aria-current=move || if current_page.get() == Page::Dashboard { Some("page") } else { None } style:color=move || if current_page.get() == Page::Dashboard { "var(--primary)" } else { "#a89984" } on:click=move |_| set_current_page.set(Page::Dashboard)>"DASHBOARD"</button>
                    <button class="nav-link" aria-current=move || if current_page.get() == Page::Rewards { Some("page") } else { None } style:color=move || if current_page.get() == Page::Rewards { "var(--primary)" } else { "#a89984" } on:click=move |_| { set_current_page.set(Page::Rewards); set_selected_month.set(None); set_selected_day.set(None); }>"WRAPPED"</button>
                    <button class="nav-link" aria-current=move || if current_page.get() == Page::Charts { Some("page") } else { None } style:color=move || if current_page.get() == Page::Charts { "var(--primary)" } else { "#a89984" } on:click=move |_| set_current_page.set(Page::Charts)>"CHARTS"</button>
                    <button class="nav-link" aria-current=move || if current_page.get() == Page::Settings { Some("page") } else { None } aria-label="Settings" style:color=move || if current_page.get() == Page::Settings { "var(--primary)" } else { "#a89984" } on:click=move |_| set_current_page.set(Page::Settings)>"⚙"</button>
                </nav>
            </header>

            <main style="display: flex; flex-direction: column; gap: 15px;">
                {move || if bowie_lookup.get().is_none() {
                    view! {
                        <div class="card" style="padding: 40px; text-align: center; border: 2px solid var(--primary);">
                            <div class="pulse-icon" style="font-size: 3rem; margin-bottom: 20px;">"⚡"</div>
                            <div style="font-weight: 900; letter-spacing: 2px; color: var(--primary);">"INITIALIZING DATABASE..."</div>
                            <div style="font-size: 0.7rem; color: #a89984; margin-top: 10px;">"Processing 10,000 recording variants..."</div>
                        </div>
                    }.into_view()
                } else {
                    match current_page.get() {
                        Page::Dashboard => view! {
                            <div style="display: flex; flex-direction: column; gap: 15px;">
                                {move || now_playing.get().map(|l| {
                                    // Default values from Listen
                                    let track_name = l.track_metadata.track_name.clone();
                                    let mut album_name = l.track_metadata.release_name.clone().unwrap_or("Unknown Album".to_string());
                                    let mut duration = l.track_metadata.additional_info.as_ref().and_then(|i| i.duration_ms).unwrap_or(0);
                                    let mut art_url: Option<String> = None;

                                    // Trusted Lookup using robust matching
                                    if let Some(lookup) = bowie_lookup.get() {
                                        // Get hint from last listen to prioritize current album
                                        let last_rg_id_string = listens.get().first().and_then(|l| {
                                            match_playing_now(&l.track_metadata, &lookup, None).map(|(_rec_id, rg_id)| rg_id)
                                        });
                                        let last_rg_id = last_rg_id_string.as_ref();

                                        // Use the shared matching logic to find the ID (mbid or name match)
                                        if let Some((rec_id, rg_id)) = match_playing_now(&l.track_metadata, &lookup, last_rg_id) {
                                            // Get trusted duration
                                            if let Some(d) = lookup.track_durations.get(&rec_id) {
                                                duration = *d;
                                            }
                                            
                                            // Get trusted album/art
                                            if let Some((title, art, _, _)) = lookup.release_groups.get(&rg_id) {
                                                album_name = title.clone();
                                                art_url = art.clone();
                                            }
                                        }
                                    }

                                    let start = playback_start.get().unwrap_or(0);
                                    let now = now_ts.get();
                                    let elapsed_ms = (now - start).max(0) * 1000;
                                    let pct = if duration > 0 { (elapsed_ms as f64 / duration as f64) * 100.0 } else { 0.0 };
                                    let pct_clamped = pct.min(100.0);
                                    let fmt_time = |ms: i64| format!("{}:{:02}", ms / 60000, (ms % 60000) / 1000);

                                    view! {
                                    <div class="card now-playing-card" style="border: 2px solid var(--primary); background: #32302f; padding: 12px 15px;">
                                        <div style="display: flex; gap: 15px; align-items: center;">
                                            {
                                                if let Some(url) = art_url {
                                                    view! { <img src=url style="width: 80px; height: 80px; border-radius: 6px; box-shadow: 0 4px 10px rgba(0,0,0,0.4); object-fit: cover;" alt="Album Art"/> }.into_view()
                                                } else {
                                                    view! { <div style="width: 80px; height: 80px; border-radius: 6px; background: rgba(255,255,255,0.05); display: flex; align-items: center; justify-content: center; font-size: 2rem;">"⚡"</div> }.into_view()
                                                }
                                            }
                                            <div style="flex: 1;">
                                                <div class="stat-label" style="color: var(--primary); font-size: 0.6rem; letter-spacing: 2px; font-weight: 900;">"LIVE ON AIR"</div>
                                                <div style="font-weight: 900; font-size: 1.3rem; color: var(--fg-color); margin: 4px 0; overflow: hidden; text-overflow: ellipsis; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical;">{track_name}</div>
                                                <div style="font-size: 0.8rem; color: #a89984; font-weight: 500;">{album_name}</div>
                                                <div style="margin-top: 8px; font-size: 0.7rem; font-family: monospace; color: var(--secondary);">
                                                    {fmt_time(elapsed_ms)} " / " {if duration > 0 { fmt_time(duration) } else { "??:??".to_string() }}
                                                </div>
                                            </div>
                                        </div>
                                        <div style="background: var(--surface); height: 4px; border-radius: 2px; margin-top: 15px; width: 100%; overflow: hidden;">
                                            <div style={format!("width: {}%; background: var(--primary); height: 100%; transition: width 1s linear;", pct_clamped)}></div>
                                        </div>
                                    </div>
                                }})}

                                <div class="card" style="border-left: 4px solid var(--accent); padding: 15px;">
                                    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px;">
                                        <span class="stat-label" style="font-size: 0.7rem;">"PROJECTION"</span>
                                        <div style="display: flex; background: var(--surface); border-radius: 4px; padding: 2px;">
                                            {["DAY", "WEEK", "MONTH", "YEAR"].iter().map(|&b| view! {
                                                <button style:background=move || if proj_basis.get() == b { "var(--accent)" } else { "transparent" } style:font-size="0.6rem" style:padding="3px 8px" on:click=move |_| set_proj_basis.set(b.to_string())>{b}</button>
                                            }).collect_view()}
                                        </div>
                                    </div>
                                    <div class="grid-container" style="grid-template-columns: repeat(4, 1fr); text-align: center;">
                                        {["DAY", "WEEK", "MONTH", "YEAR"].iter().map(|&t| view! {
                                            <div> <div class="stat-label" style="font-size: 0.5rem;">{t}</div> <div style="font-size: 1rem; font-weight: 900;">{move || format_mins(*metrics.get().projections.get(t).unwrap_or(&0))}</div> </div>
                                        }).collect_view()}
                                    </div>
                                </div>

                                {move || metrics.get().song_of_the_day.as_ref().map(|(name, album)| {
                                    let art_url = bowie_lookup.get().and_then(|l| l.release_groups.values().find(|m| &m.0 == album).and_then(|m| m.1.clone()));
                                    view! {
                                        <div class="card" style="border: 1px solid var(--secondary); background: rgba(69, 133, 136, 0.05); padding: 15px;">
                                            <div class="stat-label" style="color: var(--secondary); font-size: 0.6rem; letter-spacing: 2px; font-weight: 900; margin-bottom: 10px;">"SONG OF THE DAY"</div>
                                            <div style="display: flex; gap: 15px; align-items: center;">
                                                {art_url.map(|url| view! {
                                                    <img src=url style="width: 60px; height: 60px; border-radius: 6px; box-shadow: 0 4px 10px rgba(0,0,0,0.3);" alt=""/>
                                                })}
                                                <div style="flex: 1;">
                                                    <div style="font-weight: 900; font-size: 1.1rem; color: var(--fg-color);">{name}</div>
                                                    <div style="font-size: 0.8rem; color: #a89984;">{album}</div>
                                                    <div style="font-size: 0.6rem; color: var(--secondary); margin-top: 4px; font-style: italic;">"Time to rediscover this one."</div>
                                                </div>
                                            </div>
                                        </div>
                                    }
                                })}

                                <div class="card" style="padding: 15px;">
                                    <div class="grid-container" style="grid-template-columns: 1fr 1fr 1fr 1fr; gap: 8px; font-size: 0.75rem;">
                                        <div class="stat-label">"ROLLING"</div> <div class="stat-label" style="text-align: right">"SONGS"</div> <div class="stat-label" style="text-align: right">"ALBUMS"</div> <div class="stat-label" style="text-align: right">"TIME"</div>
                                        <div>"Today"</div> <div style="text-align: right">{move || metrics.get().counts.today}</div> <div style="text-align: right; color: var(--primary);">{move || format!("{:.2}", metrics.get().albums.today)}</div> <div style="text-align: right">{move || format_mins(metrics.get().minutes.today)}</div>
                                        <div style="font-weight: bold; border-top: 1px solid var(--surface);">"Total"</div> <div style="text-align: right; font-weight: bold; border-top: 1px solid var(--surface);">{move || metrics.get().counts.total}</div> <div style="text-align: right; font-weight: bold; border-top: 1px solid var(--surface);">{move || format!("{:.1}", metrics.get().albums.total)}</div> <div style="text-align: right; font-weight: bold; border-top: 1px solid var(--surface);">{move || format_mins(metrics.get().minutes.total)}</div>
                                    </div>
                                </div>

                                <div class="card" style="padding: 15px;">
                                    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px;">
                                        <h3 class="stat-label" style="margin: 0; font-size: 0.7rem;">"CHRONOLOGICAL"</h3>
                                        <button on:click=move |_| sync_action.dispatch(()) disabled=move || is_syncing.get() style="padding: 4px 12px; font-size: 0.65rem; font-weight: 900;"> "SYNC" </button>
                                    </div>
                                    <div style="display: flex; flex-direction: column; gap: 6px;">
                                        {move || {
                                            let now = Utc::now().timestamp();
                                            let lookup_opt = bowie_lookup.get();
                                            
                                            listens.get().iter().filter(|l| {
                                                let id = l.track_metadata.mbid_mapping.as_ref().and_then(|m| m.recording_mbid.as_ref()).or_else(|| l.track_metadata.additional_info.as_ref().and_then(|i| i.recording_mbid.as_ref()));
                                                if let (Some(lookup), Some(i)) = (&lookup_opt, id) {
                                                    lookup.recordings.contains_key(i)
                                                } else { false }
                                            }).take(display_count.get()).map(|l| {
                                                let mbid = l.track_metadata.mbid_mapping.as_ref().and_then(|m| m.recording_mbid.as_ref()).or_else(|| l.track_metadata.additional_info.as_ref().and_then(|i| i.recording_mbid.as_ref())).unwrap();
                                                let lookup = lookup_opt.as_ref().unwrap();
                                                let rg_id = lookup.recordings.get(mbid).unwrap();
                                                let (rg_title, art_url, _, _) = lookup.release_groups.get(rg_id).unwrap();
                                                
                                                // Get track duration from LB stream
                                                let dur_ms = l.track_metadata.additional_info.as_ref().and_then(|i| i.duration_ms).unwrap_or(0);
                                                let dur_display = format!("{}:{:02}", dur_ms/60000, (dur_ms%60000)/1000);
                                                
                                                view! {
                                                    <div style="background: rgba(255,255,255,0.02); padding: 8px 10px; border-radius: 6px; display: flex; gap: 12px; align-items: center; border: 1px solid #3c3836;">
                                                        {art_url.as_ref().map(|url| view! { <img src=url style="width: 40px; height: 40px; border-radius: 4px; object-fit: cover;" alt=""/> })}
                                                        <div style="flex: 1; overflow: hidden;">
                                                            <div style="font-weight: 500; font-size: 0.85rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;">{l.track_metadata.track_name.clone()}</div>
                                                            <div style="font-size: 0.6rem; color: #a89984;">{dur_display} " • " {rg_title}</div>
                                                        </div>
                                                        <div style="color: var(--accent); font-size: 0.7rem; font-weight: 900;">{format_relative_time(l.listened_at, now)}</div>
                                                    </div>
                                                }
                                            }).collect_view()
                                        }}
                                    </div>
                                </div>
                            </div>
                        }.into_view(),
                        Page::Rewards => view! { 
                            <div style="display: flex; flex-direction: column; gap: 15px;">
                                {move || match selected_month.get() {
                                    None => {
                                        let rewards = metrics.get().rewards.clone();
                                        view! {
                                            <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(250px, 1fr)); gap: 15px;">
                                                {rewards.into_iter().map(|w| {
                                                    let val = w.clone();
                                                    view! {
                                                        <div class="card" on:click={
                                                            let v_inner = val.clone();
                                                            move |_| { set_selected_month.set(Some(v_inner.clone())); }
                                                        } style="cursor: pointer; border-left: 4px solid var(--accent); padding: 15px; position: relative;">
                                                            {val.badge.as_ref().map(|b| view! {
                                                                <div style="position: absolute; top: 0; right: 0; background: var(--accent); color: var(--bg-color); font-size: 0.5rem; padding: 2px 6px; font-weight: 900;">{b}</div>
                                                            })}
                                                            <div style="font-weight: bold; color: var(--accent); font-size: 1.1rem;">{format!("{} {}", val.month_name, val.year)}</div>
                                                            <div style="font-size: 0.8rem; color: #a89984;">{val.total_scrobbles} " scrobbles"</div>
                                                        </div>
                                                    }
                                                }).collect_view()}
                                            </div> 
                                        }.into_view()
                                    },
                                    Some(w) => {
                                        let wrapped = w.clone();
                                        view! {
                                            <div style="display: flex; flex-direction: column; gap: 15px;">
                                                <div style="display: flex; justify-content: space-between; align-items: center;">
                                                    <h2 style="margin: 0; color: var(--accent); font-size: 1.2rem; font-weight: 900;">{format!("{} {} SUMMARY", wrapped.month_name.to_uppercase(), wrapped.year)}</h2>
                                                    <button on:click=move |_| { set_selected_month.set(None); } style="background: var(--surface); padding: 5px 15px; font-size: 0.7rem; font-weight: bold;">"BACK"</button>
                                                </div>
                                                
                                                {wrapped.badge.as_ref().map(|b| view! {
                                                    <div style="background: var(--accent); color: var(--bg-color); padding: 10px; border-radius: 8px; font-weight: 900; text-align: center; letter-spacing: 2px;">{b}</div>
                                                })}

                                                <div class="stat-label" style="font-size: 0.6rem; letter-spacing: 1px;">"DAILY SESSIONS"</div>
                                                <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 10px;">
                                                    {wrapped.days.iter().cloned().map(|d| {
                                                        let day_val = d.clone();
                                                        view! {
                                                            <div class="card" on:click={
                                                                let dv = day_val.clone();
                                                                move |_| {
                                                                    set_selected_day.set(Some(dv.clone()));
                                                                    set_current_page.set(Page::DayDetail);
                                                                }
                                                            } style="padding: 12px; background: var(--surface); cursor: pointer; border-left: 3px solid var(--primary); position: relative;">
                                                                {day_val.badge.as_ref().map(|b| view! {
                                                                    <div style="position: absolute; top: 0; right: 0; background: var(--primary); color: var(--bg-color); font-size: 0.4rem; padding: 1px 4px; font-weight: 900;">{b}</div>
                                                                })}
                                                                <div style="font-weight: 900; font-size: 0.85rem; color: var(--fg-color);">{day_val.date_label.clone()}</div>
                                                                <div style="font-size: 0.7rem; margin-top: 4px; color: #a89984;">{day_val.scrobbles} " tracks"</div>
                                                            </div>
                                                        }
                                                    }).collect_view()}
                                                </div>
                                            </div>
                                        }.into_view()
                                    }
                                }}
                            </div>
                        }.into_view(),
                        Page::DayDetail => view! {
                            <div style="display: flex; flex-direction: column; gap: 15px;">
                                {move || selected_day.get().map(|d| {
                                    let day_label = d.date_label.clone();
                                    view! {
                                        <div style="display: flex; justify-content: space-between; align-items: center;">
                                            <h2 style="margin: 0; color: var(--primary); font-size: 1.2rem; font-weight: 900;">{day_label.to_uppercase()}</h2>
                                            <button on:click=move |_| { set_current_page.set(Page::Rewards); } style="background: var(--surface); padding: 5px 15px; font-size: 0.7rem; font-weight: bold;">"BACK"</button>
                                        </div>

                                        {d.badge.as_ref().map(|b| view! {
                                            <div style="background: var(--primary); color: var(--bg-color); padding: 10px; border-radius: 8px; font-weight: 900; text-align: center; letter-spacing: 2px;">{b}</div>
                                        })}

                                        <div class="card" style="padding: 20px; border-top: 4px solid var(--primary);">
                                            <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 30px;">
                                                <div>
                                                    <div class="stat-label" style="margin-bottom: 12px;">"ALBUM AFFINITY"</div>
                                                    {d.top_albums.iter().cloned().map(|(n, c, _)| {
                                                        let lookup_opt = bowie_lookup.get();
                                                        let art = lookup_opt.and_then(|l| l.release_groups.values().find(|m| &m.0 == &n).and_then(|m| m.1.clone()));
                                                        view! { 
                                                            <div style="font-size: 0.8rem; margin-bottom: 10px; display: flex; align-items: center; gap: 10px;">
                                                                {art.map(|url| view! { <img src=url style="width: 30px; height: 30px; border-radius: 2px;" alt=""/> })}
                                                                <div style="flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{n}</div> 
                                                                <div style="color: var(--primary); font-weight: bold;">{format!("{:.2}", c)}</div>
                                                            </div> 
                                                        }
                                                    }).collect_view()}
                                                </div>
                                                <div>
                                                    <div class="stat-label" style="margin-bottom: 12px;">"TRACK FOCUS"</div>
                                                    {d.top_tracks.iter().cloned().map(|(n, c, _)| view! { 
                                                        <div style="font-size: 0.8rem; margin-bottom: 10px; display: flex; justify-content: space-between; border-bottom: 1px solid var(--surface); padding-bottom: 4px;">
                                                            <span style="overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{n}</span> 
                                                            <span style="color: var(--accent); font-weight: bold; margin-left: 8px;">"x"{c}</span>
                                                        </div> 
                                                    }).collect_view()}
                                                </div>
                                            </div>
                                            <div style="margin-top: 20px; padding-top: 15px; border-top: 1px solid var(--surface); color: #a89984; font-size: 0.75rem;">
                                                {format!("Total listening time: {} minutes across {} tracks.", d.minutes, d.scrobbles)}
                                            </div>
                                        </div>
                                    }
                                })}
                            </div>
                        }.into_view(),
                        Page::Charts => view! { <ListeningHistoryChart metrics=metrics /> }.into_view(),
                        Page::Settings => view! {
                            <div class="card" style="max-width: 450px; margin: 0 auto; width: 100%; gap: 15px; padding: 25px;">
                                <h3 style="margin: 0; font-weight: 900; color: var(--primary);">"ACCOUNT"</h3>
                                <div style="display: flex; flex-direction: column; gap: 10px;">
                                    <label for="settings_username" class="stat-label" style="font-size: 0.6rem;">"USERNAME"</label>
                                    <input id="settings_username" type="text" style="padding: 12px; font-weight: bold;" prop:value=username on:input=move |ev| { let v = event_target_value(&ev); set_username.set(v.clone()); } />
                                    <button style="background: var(--secondary); padding: 15px; margin-top: 20px; font-weight: 900;" on:click=move |_| deep_sync.dispatch(())> "SYNC HISTORY" </button>
                                </div>
                            </div>
                        }.into_view(),
                    }
                }}
            </main>

            <footer style="margin-top: 40px; padding: 20px; text-align: center; border-top: 1px solid var(--surface); display: flex; flex-direction: column; gap: 10px; font-size: 0.7rem; color: #a89984;">
                <div style="font-weight: 900; letter-spacing: 2px; color: var(--primary);">"STATION TO STATION"</div>
                <div style="display: flex; justify-content: center; gap: 20px;">
                    <a href="https://github.com/caminante-blanco/bowie-tracker" target="_blank" rel="noopener noreferrer" style="color: inherit; text-decoration: none; font-weight: bold;">"SOURCE CODE"</a>
                    <a href="https://github.com/caminante-blanco" target="_blank" rel="noopener noreferrer" style="color: inherit; text-decoration: none; font-weight: bold;">"CAMINANTE-BLANCO"</a>
                </div>
            </footer>
        </div>
    }
}
