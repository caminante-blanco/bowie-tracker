use leptos::*;
use chrono::{Utc, TimeZone};
use std::collections::{HashMap, HashSet};

use bowie_tracker::models::{Listen, ListenBrainzResponse, PlayingNowResponse, MBReleaseGroupResponse, BowieDatabase, PlayingNowListen};
use bowie_tracker::analytics::{calculate_metrics, is_bowie, is_bowie_meta, format_relative_time, MonthlyWrapped, DayStats};
use bowie_tracker::db::{init_db, add_listens, get_all_listens, get_max_timestamp, get_all_album_metadata, save_album_metadata, AlbumMetadata};
use bowie_tracker::charts::ListeningHistoryChart;

#[derive(Clone, Copy, PartialEq)]
enum Page { Dashboard, Rewards, Charts, Settings, DayDetail }

fn main() {
    console_error_panic_hook::set_once();
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
    
    // Album Track Count Cache
    let (track_counts, set_track_counts) = create_signal(HashMap::<String, usize>::new());
    let (bowie_db, set_bowie_db) = create_signal(None::<BowieDatabase>);

    let bowie_mbids = create_memo(move |_| {
        bowie_db.get().map(|db| {
            db.release_groups.values()
                .flat_map(|rg| rg.tracks.iter().map(|t| t.id.clone()))
                .collect::<HashSet<String>>()
        }).unwrap_or_default()
    });

    create_effect(move |_| {
        spawn_local(async move {
            let url = "/bowie_metadata.json";
            web_sys::console::log_1(&format!("Fetching Bowie metadata from {}...", url).into());
            match reqwest::get(url).await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        match resp.json::<BowieDatabase>().await {
                            Ok(db) => {
                                web_sys::console::log_1(&format!("Bowie metadata loaded successfully ({} albums)", db.release_groups.len()).into());
                                set_bowie_db.set(Some(db));
                            },
                            Err(e) => web_sys::console::log_1(&format!("Failed to parse Bowie metadata JSON: {}", e).into()),
                        }
                    } else {
                        web_sys::console::log_1(&format!("Failed to fetch Bowie metadata: HTTP {}", resp.status()).into());
                    }
                },
                Err(e) => web_sys::console::log_1(&format!("Network error fetching Bowie metadata: {}", e).into()),
            }
        });
    });

    let (current_page, set_current_page) = create_signal(Page::Dashboard);
    let (proj_basis, set_proj_basis) = create_signal("DAY".to_string());
    let (is_setup, set_is_setup) = create_signal(true); 
    let (display_count, set_display_count) = create_signal(25);

    let (selected_month, set_selected_month) = create_signal(None::<MonthlyWrapped>);
    let (selected_day, set_selected_day) = create_signal(None::<DayStats>);

    let metrics = create_memo(move |_| {
        calculate_metrics(&listens.get(), Utc::now(), &proj_basis.get(), &track_counts.get(), bowie_db.get().as_ref())
    });

    let get_album_art = move |album_name: String| {
        if let Some(db) = bowie_db.get() {
            let name_low = album_name.to_lowercase();
            for rg in db.release_groups.values() {
                if rg.title.to_lowercase() == name_low {
                    return rg.image_url.clone();
                }
            }
        }
        None
    };

    let refresh_data = move || {
        spawn_local(async move {
            if let Ok(db) = init_db().await {
                if let Ok(mut all) = get_all_listens(&db).await {
                    all.sort_by(|a, b| b.listened_at.cmp(&a.listened_at));
                    let listens_vec: Vec<Listen> = all;
                    set_listens.set(listens_vec);
                }
                if let Ok(counts) = get_all_album_metadata(&db).await {
                    set_track_counts.set(counts);
                }
            }
        });
    };

    let perform_sync = move || {
        let user = username.get_untracked();
        let user_token = token.get_untracked();
        let mbids = bowie_mbids.get_untracked();
        if user.is_empty() || is_syncing.get_untracked() { return; }
        
        spawn_local(async move {
            set_is_syncing.set(true);
            set_status.set("SYNCING".to_string());
            if let Ok(db) = init_db().await {
                let latest_ts = get_max_timestamp(&db).await.unwrap_or(None).unwrap_or(0);
                let url = format!("https://api.listenbrainz.org/1/user/{}/listens?count=1000&min_ts={}", user, latest_ts + 1);
                let mut req = reqwest::Client::new().get(&url);
                if !user_token.is_empty() { req = req.header("Authorization", format!("Token {}", user_token)); }
                if let Ok(resp) = req.send().await {
                    if let Ok(json) = resp.json::<ListenBrainzResponse>().await {
                        let batch: Vec<Listen> = json.payload.listens;
                        if !batch.is_empty() {
                            let _ = add_listens(&db, batch).await;
                            refresh_data();
                        }
                    }
                }
                let np_url = format!("https://api.listenbrainz.org/1/user/{}/playing-now", user);
                let mut np_req = reqwest::Client::new().get(&np_url);
                if !user_token.is_empty() { np_req = np_req.header("Authorization", format!("Token {}", user_token)); }
                if let Ok(resp) = np_req.send().await {
                    if let Ok(json) = resp.json::<PlayingNowResponse>().await {
                        let mbids_ref = &mbids;
                        set_now_playing.set(json.payload.listens.into_iter().find(|l| is_bowie_meta(&l.track_metadata, mbids_ref)));
                    }
                }
            }
            set_is_syncing.set(false);
            set_status.set("Ready".to_string());
        });
    };

    // Auto-discovery of missing album track counts
    create_effect(move |_| {
        let current_listens = listens.get();
        let counts = track_counts.get_untracked();
        
        for listen in current_listens {
            if let Some(rg_mbid) = listen.track_metadata.additional_info.as_ref().and_then(|i| i.release_group_mbid.as_ref()) {
                let album_name = listen.track_metadata.mbid_mapping.as_ref().and_then(|m| m.release_name.clone()).or_else(|| listen.track_metadata.release_name.clone()).unwrap_or_default();
                if !album_name.is_empty() && !counts.contains_key(&album_name) {
                    // Logic to fetch from MB and save
                    let rg_id = rg_mbid.clone();
                    let name = album_name.clone();
                    spawn_local(async move {
                        let url = format!("https://musicbrainz.org/ws/2/release-group/{}?inc=releases&fmt=json", rg_id);
                        let client = reqwest::Client::builder().user_agent("ZiggyTracker/0.1.0 ( walkercwhite@gmail.com )").build().unwrap();
                        if let Ok(resp) = client.get(&url).send().await {
                            if let Ok(json) = resp.json::<MBReleaseGroupResponse>().await {
                                if let Some(first) = json.releases.first() {
                                    let count = first.track_count;
                                    if let Ok(db) = init_db().await {
                                        let _ = save_album_metadata(&db, AlbumMetadata { release_group_mbid: name, track_count: count }).await;
                                        // No easy way to refresh just the counts signal without a reload or complex logic
                                    }
                                }
                            }
                        }
                    });
                }
            }
        }
    });

    create_effect(move |_| {
        let handle = gloo_timers::callback::Interval::new(30_000, move || {
            perform_sync();
        });
        move || drop(handle)
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

    let sync_action = create_action(move |_: &()| {
        async move { perform_sync(); }
    });

    let deep_sync = create_action(move |_: &()| {
        let user = username.get();
        let user_token = token.get();
        async move {
            if user.is_empty() { return; }
            set_is_syncing.set(true);
            if let Ok(db) = init_db().await {
                let current_listens: Vec<Listen> = get_all_listens(&db).await.unwrap_or_default();
                let mut oldest_ts = current_listens.iter().map(|l| l.listened_at).min().unwrap_or_else(|| Utc::now().timestamp());
                for i in 0..20 {
                    set_status.set(format!("DEEP SYNC {}/20", i+1));
                    let url = format!("https://api.listenbrainz.org/1/user/{}/listens?count=1000&max_ts={}", user, oldest_ts - 1);
                    let mut req = reqwest::Client::new().get(&url);
                    if !user_token.is_empty() { req = req.header("Authorization", format!("Token {}", user_token)); }
                    if let Ok(resp) = req.send().await {
                        if let Ok(json) = resp.json::<ListenBrainzResponse>().await {
                            let batch: Vec<Listen> = json.payload.listens;
                            if batch.is_empty() { break; }
                            oldest_ts = batch.iter().map(|l| l.listened_at).min().unwrap_or(oldest_ts);
                            let _ = add_listens(&db, batch).await;
                            refresh_data(); // Refresh UI after each batch
                        } else { break; }
                    } else { break; }
                    gloo_timers::future::sleep(std::time::Duration::from_millis(300)).await;
                }
                set_status.set("HISTORY READY".to_string());
            }
            set_is_syncing.set(false);
        }
    });

    let format_mins = |mins: i64| {
        if mins >= 60 { format!("{:.1}h", mins as f64 / 60.0) } else { format!("{}m", mins) }
    };

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
                        <button on:click=move |_| { 
                            perform_sync(); 
                            deep_sync.dispatch(());
                            set_is_setup.set(true); 
                        } style="padding: 20px; background: var(--primary); font-weight: bold; letter-spacing: 2px;">"INITIALIZE"</button>
                    </div>
                </div>
            }.into_view() } else { view! { <div></div> }.into_view() }}

            <header style="background: var(--card-bg); padding: 12px; border-radius: 12px; margin-bottom: 15px; position: sticky; top: 10px; z-index: 50; display: flex; justify-content: space-between; align-items: center; box-shadow: 0 4px 15px rgba(0,0,0,0.4);">
                <div style="display: flex; align-items: center; gap: 15px;">
                    <h1 style="color: var(--primary); margin: 0; font-size: 1.4rem; font-weight: 900; letter-spacing: 3px;">"ZIGGY"</h1>
                    <span style="font-size: 0.6rem; color: var(--accent); font-weight: 900; letter-spacing: 1px; background: var(--surface); padding: 2px 8px; border-radius: 4px;">{move || status.get().to_uppercase()}</span>
                </div>
                <nav style="display: flex; gap: 15px; font-size: 0.8rem; font-weight: bold; letter-spacing: 1px;" role="navigation" aria-label="Main menu">
                    <button class="nav-link" aria-current=move || if current_page.get() == Page::Dashboard { Some("page") } else { None } style:color=move || if current_page.get() == Page::Dashboard { "var(--primary)" } else { "#a89984" } on:click=move |_| set_current_page.set(Page::Dashboard)>"DASH"</button>
                    <button class="nav-link" aria-current=move || if current_page.get() == Page::Rewards { Some("page") } else { None } style:color=move || if current_page.get() == Page::Rewards { "var(--primary)" } else { "#a89984" } on:click=move |_| { set_current_page.set(Page::Rewards); set_selected_month.set(None); set_selected_day.set(None); }>"WRAPPED"</button>
                    <button class="nav-link" aria-current=move || if current_page.get() == Page::Charts { Some("page") } else { None } style:color=move || if current_page.get() == Page::Charts { "var(--primary)" } else { "#a89984" } on:click=move |_| set_current_page.set(Page::Charts)>"CHARTS"</button>
                    <button class="nav-link" aria-current=move || if current_page.get() == Page::Settings { Some("page") } else { None } aria-label="Settings" style:color=move || if current_page.get() == Page::Settings { "var(--primary)" } else { "#a89984" } on:click=move |_| set_current_page.set(Page::Settings)>"⚙"</button>
                </nav>
            </header>

            <main style="display: flex; flex-direction: column; gap: 15px;">
                {move || match current_page.get() {
                    Page::Dashboard => view! {
                        <div style="display: flex; flex-direction: column; gap: 15px;">
                            {move || now_playing.get().map(|l| view! {
                                <div class="card now-playing-card" style="border: 2px solid var(--primary); background: #32302f; padding: 12px 15px;">
                                    <div style="display: flex; justify-content: space-between; align-items: center;">
                                        <div>
                                            <div class="stat-label" style="color: var(--primary); font-size: 0.6rem; letter-spacing: 2px; font-weight: 900;">"LIVE ON AIR"</div>
                                            <div style="font-weight: 900; font-size: 1.3rem; color: var(--fg-color); margin: 4px 0;">{l.track_metadata.track_name}</div>
                                            <div style="font-size: 0.8rem; color: #a89984; font-weight: 500;">"David Bowie"</div>
                                        </div>
                                        <div class="pulse-icon" style="width: 12px; height: 12px; background: var(--primary); border-radius: 50%; box-shadow: 0 0 10px var(--primary);"></div>
                                    </div>
                                </div>
                            })}

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
                                let art = get_album_art(album.clone());
                                view! {
                                    <div class="card" style="border: 1px solid var(--secondary); background: rgba(69, 133, 136, 0.05); padding: 15px;">
                                        <div class="stat-label" style="color: var(--secondary); font-size: 0.6rem; letter-spacing: 2px; font-weight: 900; margin-bottom: 10px;">"SONG OF THE DAY"</div>
                                        <div style="display: flex; gap: 15px; align-items: center;">
                                            {art.map(|url| view! {
                                                <img src=url style="width: 60px; height: 60px; border-radius: 6px; box-shadow: 0 4px 10px rgba(0,0,0,0.3);" alt="album art"/>
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
                                <div class="stat-label" style="margin-bottom: 8px;">"RECENT PERFORMANCE"</div>
                                <table style="width: 100%; border-collapse: collapse; font-size: 0.75rem;">
                                    <tbody>{move || {
                                        metrics.get().history.iter().take(5).cloned().map(|day| view! {
                                            <tr style="border-bottom: 1px solid #3c3836;">
                                                <td style="padding: 8px 0;">{day.date_label}</td>
                                                <td style="text-align: right; color: var(--primary); font-weight: bold;">{format!("{:.2}", day.albums_completed)}</td>
                                                <td style="text-align: right; padding: 0 10px;">{format_mins(day.minutes)}</td>
                                                <td style="color: #d5c4a1; font-style: italic; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 120px;">{day.favorite_album}</td>
                                            </tr>
                                        }).collect_view()
                                    }}</tbody>
                                </table>
                            </div>

                            <div class="card" style="padding: 15px;">
                                <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px;">
                                    <h3 class="stat-label" style="margin: 0; font-size: 0.7rem;">"CHRONOLOGICAL"</h3>
                                    <button on:click=move |_| sync_action.dispatch(()) disabled=move || is_syncing.get() style="padding: 4px 12px; font-size: 0.65rem; font-weight: 900;"> "SYNC" </button>
                                </div>
                                <div style="display: flex; flex-direction: column; gap: 6px;">
                                    {move || {
                                        let now = Utc::now().timestamp();
                                        let mbids = bowie_mbids.get();
                                        
                                        // Create quick lookup maps
                                        let mut durations = HashMap::new();
                                        let mut title_durations = HashMap::new();
                                        if let Some(db) = bowie_db.get() {
                                            for rg in db.release_groups.values() {
                                                for track in &rg.tracks {
                                                    let m_entry = durations.entry(track.id.clone()).or_insert(0);
                                                    if track.duration_ms > *m_entry { *m_entry = track.duration_ms; }
                                                    
                                                    let t_entry = title_durations.entry(track.title.clone()).or_insert(0);
                                                    if track.duration_ms > *t_entry { *t_entry = track.duration_ms; }
                                                }
                                            }
                                        }

                                        listens.get().iter().filter(|l| is_bowie(l, &mbids)).take(display_count.get()).map(|l| {
                                            let track_name = l.track_metadata.mbid_mapping.as_ref().and_then(|m| m.recording_name.clone()).unwrap_or_else(|| l.track_metadata.track_name.clone());
                                            let album_name = l.track_metadata.mbid_mapping.as_ref().and_then(|m| m.release_name.clone()).or_else(|| l.track_metadata.release_name.clone()).unwrap_or_default();
                                            let art_url = get_album_art(album_name);

                                            let mbid = l.track_metadata.mbid_mapping.as_ref()
                                                .and_then(|m| m.recording_mbid.as_ref())
                                                .or_else(|| l.track_metadata.additional_info.as_ref().and_then(|i| i.recording_mbid.as_ref()));

                                            let dur_ms = mbid.and_then(|id| durations.get(id).cloned())
                                                .or_else(|| title_durations.get(&track_name).cloned())
                                                .unwrap_or(0);

                                            let dur_display = if dur_ms == 0 { 
                                                if bowie_db.get().is_none() { "loading metadata...".to_string() } else { "no mbid match".to_string() }
                                            } else { 
                                                format!("{}:{:02}", dur_ms/60000, (dur_ms%60000)/1000) 
                                            };
                                            let time = Utc.timestamp_opt(l.listened_at, 0).unwrap().format("%I:%M %p").to_string();
                                            view! {
                                                <div style="background: rgba(255,255,255,0.02); padding: 8px 10px; border-radius: 6px; display: flex; gap: 12px; align-items: center; border: 1px solid #3c3836;">
                                                    {art_url.map(|url| view! {
                                                        <img src=url style="width: 40px; height: 40px; border-radius: 4px; object-fit: cover;" alt="album art"/>
                                                    })}
                                                    <div style="flex: 1; overflow: hidden;">
                                                        <div style="font-weight: 500; font-size: 0.85rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;">{track_name}</div>
                                                        <div style="font-size: 0.6rem; color: #a89984;">{time} " • " {dur_display}</div>
                                                    </div>
                                                    <div style="color: var(--accent); font-size: 0.7rem; font-weight: 900; margin-left: 8px;">{format_relative_time(l.listened_at, now)}</div>
                                                </div>
                                            }
                                        }).collect_view()
                                    }}
                                </div>
                                <button on:click=move |_| set_display_count.update(|c| *c += 50) style="width: 100%; margin-top: 15px; background: var(--card-bg); font-size: 0.75rem; border: 1px solid var(--surface); color: #a89984; font-weight: bold;"> "LOAD MORE" </button>
                            </div>
                        </div>
                    }.into_view(),

                    Page::Rewards => view! {
                        <div style="display: flex; flex-direction: column; gap: 15px;">
                            <div style="display: flex; gap: 10px; overflow-x: auto; padding-bottom: 10px;">
                                {move || metrics.get().insights.iter().cloned().map(|i| view! {
                                    <div class="card" style="min-width: 200px; padding: 10px; border-top: 3px solid var(--accent);">
                                        <div class="stat-label" style="font-size: 0.6rem;">{i.title.to_uppercase()}</div>
                                        <div style="font-weight: 900; color: var(--accent);">{i.value}</div>
                                        <div style="font-size: 0.65rem; color: #a89984;">{i.description}</div>
                                    </div>
                                }).collect_view()}
                            </div>

                            {move || match selected_month.get() {
                                None => {
                                    let rewards = metrics.get().rewards.clone();
                                    view! {
                                        <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(250px, 1fr)); gap: 15px;">
                                            {rewards.into_iter().map(|w| {
                                                let val = w.clone();
                                                view! {
                                                    <div class="card" on:click={
                                                        let v = val.clone();
                                                        move |_| set_selected_month.set(Some(v.clone()))
                                                    } style="cursor: pointer; border-left: 4px solid var(--accent); padding: 15px; position: relative;">
                                                        {val.badge.as_ref().map(|b| view! {
                                                            <div style="position: absolute; top: 0; right: 0; background: var(--accent); color: var(--bg-color); font-size: 0.5rem; padding: 2px 6px; font-weight: 900;">{b}</div>
                                                        })}
                                                        <div style="font-weight: bold; color: var(--accent); font-size: 1.1rem;">{format!("{} {}", val.month_name, val.year)}</div>
                                                        <div style="font-size: 0.8rem; margin: 5px 0; color: #a89984;">{val.total_scrobbles} " scrobbles"</div>
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

                                    <div class="card" style="padding: 20px; border-top: 4px solid var(--primary);">
                                        <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 30px;">
                                            <div>
                                                <div class="stat-label" style="margin-bottom: 12px;">"ALBUM AFFINITY"</div>
                                                {d.top_albums.iter().cloned().map(|(n, c, _)| {
                                                    let art = get_album_art(n.clone());
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
                                <input id="settings_username" type="text" style="padding: 12px; font-weight: bold;" prop:value=username on:input=move |ev| { let v = event_target_value(&ev); set_username.set(v.clone()); if let Some(win) = web_sys::window() { if let Ok(Some(s)) = win.local_storage() { let _ = s.set_item("lb_username", &v); } } } />
                                <label for="settings_token" class="stat-label" style="font-size: 0.6rem; margin-top: 10px;">"AUTH TOKEN"</label>
                                <input id="settings_token" type="password" style="padding: 12px; font-weight: bold;" prop:value=token on:input=move |ev| { let v = event_target_value(&ev); set_token.set(v.clone()); if let Some(win) = web_sys::window() { if let Ok(Some(s)) = win.local_storage() { let _ = s.set_item("lb_username", &v); } } } />
                            </div>
                            <button style="background: var(--secondary); padding: 15px; margin-top: 20px; font-weight: 900; letter-spacing: 1px;" on:click=move |_| deep_sync.dispatch(()) disabled=move || is_syncing.get()> "TRIGGER MANUAL DEEP SYNC" </button>
                        </div>
                    }.into_view(),
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
