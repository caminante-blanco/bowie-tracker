//! Analytics and Bowie-specific filtering logic for Ziggy.
use chrono::{DateTime, Utc, TimeZone, Datelike, Timelike, Duration};
use std::collections::{HashMap, HashSet};
use crate::models::{Listen, BowieDatabase};
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Default)]
pub struct DashboardMetrics {
    pub counts: PeriodMetric<usize>,
    pub albums: PeriodMetric<f64>, 
    pub minutes: PeriodMetric<i64>,
    pub projections: HashMap<String, i64>,
    pub last_listen_display: String,
    pub favorite_album_today: String,
    pub song_of_the_day: Option<(String, String)>,
    pub history: Vec<DayStats>,
    pub rewards: Vec<MonthlyWrapped>,
    pub insights: Vec<Insight>,
    
    // Chart Data
    pub yearly_distribution: Vec<(i32, usize)>,
    pub album_completion: Vec<(String, f64, Option<String>)>,
    pub monthly_volume: Vec<(String, usize)>,
    pub track_time_leaderboard: Vec<(String, i64)>,
    pub hourly_activity: Vec<(u32, usize)>,
    pub type_distribution: Vec<(String, usize)>,
    pub discovery_timeline: Vec<(i64, usize)>,
    pub consistency_grid: Vec<(i64, usize)>,
    pub album_weight: Vec<(String, usize, Option<String>)>,
    pub forgotten_classics: Vec<(String, i64, usize)>,
}

#[derive(Clone, PartialEq, Default)]
pub struct PeriodMetric<T> {
    pub last_hour: T,
    pub today: T,
    pub week: T,
    pub month: T,
    pub year: T,
    pub total: T,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DayStats {
    pub date_label: String,
    pub timestamp: i64,
    pub albums_completed: f64,
    pub minutes: i64,
    pub scrobbles: usize,
    pub favorite_album: String,
    pub top_albums: Vec<(String, f64, i64)>, 
    pub top_tracks: Vec<(String, usize, i64)>,
    pub badge: Option<String>,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MonthlyWrapped {
    pub year: i32,
    pub month: u32,
    pub month_name: String,
    pub total_scrobbles: usize,
    pub total_minutes: i64,
    pub top_album: String,
    pub top_track: String,
    pub top_albums: Vec<(String, f64, i64)>,
    pub top_tracks: Vec<(String, usize, i64)>,
    pub days: Vec<DayStats>,
    pub badge: Option<String>,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Insight {
    pub title: String,
    pub value: String,
    pub description: String,
}

#[derive(Default)]
struct DayWork {
    scrobbles: usize,
    ms: i64,
    album_counts: HashMap<String, usize>,
    track_counts: HashMap<String, usize>,
    album_ms: HashMap<String, i64>,
    track_ms: HashMap<String, i64>,
}

pub fn calculate_metrics(
    listens: &[Listen], 
    now: DateTime<Utc>, 
    _basis: &str, 
    external_counts: &HashMap<String, usize>,
    bowie_db: Option<&BowieDatabase>
) -> DashboardMetrics {
    let mut bowie_mbids = HashSet::new();
    let mut bowie_durations = HashMap::new();
    let mut bowie_title_durations = HashMap::new();
    let mut recording_to_rg_title = HashMap::new();

    if let Some(db) = bowie_db {
        for rg in db.release_groups.values() {
            for track in &rg.tracks {
                bowie_mbids.insert(track.id.clone());
                recording_to_rg_title.insert(track.id.clone(), rg.title.clone());
                
                let m_entry = bowie_durations.entry(track.id.clone()).or_insert(0);
                if track.duration_ms > *m_entry { *m_entry = track.duration_ms; }
                
                let t_entry = bowie_title_durations.entry(track.title.clone()).or_insert(0);
                if track.duration_ms > *t_entry { *t_entry = track.duration_ms; }
            }
        }
    }

    // ... (filtering)

    for listen in &sorted_listens {
        // ... (aggregation)

        let mbid = listen.track_metadata.mbid_mapping.as_ref()
            .and_then(|m| m.recording_mbid.as_ref())
            .or_else(|| listen.track_metadata.additional_info.as_ref().and_then(|i| i.recording_mbid.as_ref()));

        let track_name = listen.track_metadata.track_name.clone();
        
        // Unify album name using Release Group if MBID matches
        let album_name = mbid.and_then(|id| recording_to_rg_title.get(id).cloned())
            .or_else(|| listen.track_metadata.mbid_mapping.as_ref().and_then(|m| m.release_name.clone()))
            .or_else(|| listen.track_metadata.release_name.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        let duration_ms = mbid.and_then(|id| bowie_durations.get(id).cloned())
            .or_else(|| bowie_title_durations.get(&track_name).cloned())
            .unwrap_or(0);

        let min = duration_ms / 60000;

        for work in [d_work, m_work] {
            work.ms += duration_ms;
            work.scrobbles += 1;
            *work.album_counts.entry(album_name.clone()).or_insert(0) += 1;
            *work.track_counts.entry(track_name.clone()).or_insert(0) += 1;
            *work.album_ms.entry(album_name.clone()).or_insert(0) += min;
            *work.track_ms.entry(track_name.clone()).or_insert(0) += min;
        }

        if ts > now_ts - 3600 { metrics.counts.last_hour += 1; metrics.minutes.last_hour += min; }
        if ts >= today_start_ts { metrics.counts.today += 1; metrics.minutes.today += min; }
        if ts >= week_start_ts { metrics.counts.week += 1; metrics.minutes.week += min; }
        if ts >= month_start_ts { metrics.counts.month += 1; metrics.minutes.month += min; }
        if ts >= year_start_ts { metrics.counts.year += 1; metrics.minutes.year += min; }
        metrics.counts.total += 1;
        metrics.minutes.total += min;

        // Discovery
        if let Some(id) = mbid {
            if unique_mbids_seen.insert(id.clone()) {
                discovery_points.push((ts, unique_mbids_seen.len()));
            }
            album_unique_tracks.entry(album_name.clone()).or_insert_with(HashSet::new).insert(id.clone());
        }

        *track_minutes.entry(track_name.clone()).or_insert(0) += min;
        *hour_map.entry(dt.hour()).or_insert(0) += 1;
        *album_scrobbles.entry(album_name.clone()).or_insert(0) += 1;
        *year_map.entry(dt.year()).or_insert(0) += 1;
        *total_count_map.entry(track_name.clone()).or_insert(0) += 1;
        let ls = last_seen_map.entry(track_name.clone()).or_insert(0);
        if ts > *ls { *ls = ts; }

        if let Some(db) = bowie_db {
            for rg in db.release_groups.values() {
                if rg.title == album_name || (mbid.is_some() && rg.tracks.iter().any(|t| Some(&t.id) == mbid)) {
                    if let Some(t) = &rg.release_type {
                        *type_map.entry(t.clone()).or_insert(0) += 1;
                    }
                    break;
                }
            }
        }
    }

    let mut day_stats_map: HashMap<i64, DayStats> = HashMap::new();
    for (day_ts, work) in &day_aggregates {
        let mut stats = DayStats {
            timestamp: *day_ts,
            date_label: Utc.timestamp_opt(*day_ts, 0).unwrap().format("%a, %b %d").to_string(),
            albums_completed: calculate_total_completion(work, external_counts, bowie_db),
            minutes: work.ms / 60000,
            scrobbles: work.scrobbles,
            ..Default::default()
        };
        stats.top_albums = get_top_albums(&work.album_counts, &work.album_ms, 5, external_counts, bowie_db);
        stats.top_tracks = get_top_items(&work.track_counts, &work.track_ms, 5);
        stats.favorite_album = stats.top_albums.first().map(|a| a.0.clone()).unwrap_or_default();
        day_stats_map.insert(*day_ts, stats);
    }

    let mut sorted_days_by_time: Vec<_> = day_stats_map.values_mut().collect();
    sorted_days_by_time.sort_by(|a, b| b.minutes.cmp(&a.minutes));
    for (i, d) in sorted_days_by_time.iter_mut().enumerate() {
        d.badge = match i {
            0 => Some("PEAK SESSION".to_string()),
            1 => Some("HIGH ACTIVITY".to_string()),
            _ => None
        };
    }

    for ((year, month), work) in &monthly_wrapped_map {
        let mut wrapped = MonthlyWrapped {
            year: *year,
            month: *month,
            month_name: format_month(*month).to_string(),
            total_scrobbles: work.scrobbles,
            total_minutes: work.ms / 60000,
            ..Default::default()
        };
        wrapped.top_albums = get_top_albums(&work.album_counts, &work.album_ms, 10, external_counts, bowie_db);
        wrapped.top_tracks = get_top_items(&work.track_counts, &work.track_ms, 10);
        wrapped.top_album = wrapped.top_albums.first().map(|a| a.0.clone()).unwrap_or_default();
        wrapped.top_track = wrapped.top_tracks.first().map(|a| a.0.clone()).unwrap_or_default();
        
        let mut month_days: Vec<_> = day_stats_map.values()
            .filter(|d| {
                let dt = Utc.timestamp_opt(d.timestamp, 0).unwrap();
                dt.year() == *year && dt.month() == *month
            })
            .cloned()
            .collect();
        month_days.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        wrapped.days = month_days;
        metrics.rewards.push(wrapped);
    }
    metrics.rewards.sort_by(|a, b| b.year.cmp(&a.year).then(b.month.cmp(&a.month)));

    let mut sorted_months_by_time = metrics.rewards.clone();
    sorted_months_by_time.sort_by(|a, b| b.total_minutes.cmp(&a.total_minutes));
    for (i, w) in sorted_months_by_time.iter().enumerate() {
        if let Some(month) = metrics.rewards.iter_mut().find(|m| m.year == w.year && m.month == w.month) {
            month.badge = match i {
                0 => Some("MILESTONE MONTH".to_string()),
                1 => Some("TOP PERIOD".to_string()),
                _ => None
            };
        }
    }

    let mut history_list: Vec<_> = day_stats_map.values().cloned().collect();
    history_list.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    metrics.history = history_list;

    if let Some(today) = day_stats_map.get(&today_start_ts) {
        metrics.albums.today = today.albums_completed;
        metrics.favorite_album_today = today.favorite_album.clone();
    }

    metrics.albums.total = metrics.history.iter().map(|d| d.albums_completed).sum();
    metrics.albums.week = metrics.history.iter().filter(|d| d.timestamp >= week_start_ts).map(|d| d.albums_completed).sum();
    metrics.albums.month = metrics.history.iter().filter(|d| d.timestamp >= month_start_ts).map(|d| d.albums_completed).sum();
    metrics.albums.year = metrics.history.iter().filter(|d| d.timestamp >= year_start_ts).map(|d| d.albums_completed).sum();

    if metrics.rewards.len() > 1 {
        let mut s_w = metrics.rewards.clone();
        s_w.sort_by(|a, b| b.total_minutes.cmp(&a.total_minutes));
        metrics.insights.push(Insight {
            title: "Top Period".to_string(),
            value: format!("{} {}", s_w[0].month_name, s_w[0].year),
            description: format!("Total: {}h", s_w[0].total_minutes / 60),
        });
    }

    let velocity = (metrics.minutes.week as f64) / (7.0 * 86400.0);
    metrics.projections.insert("DAY".to_string(), (velocity * 86400.0) as i64);
    metrics.projections.insert("WEEK".to_string(), (velocity * 7.0 * 86400.0) as i64);
    metrics.projections.insert("MONTH".to_string(), (velocity * 30.0 * 86400.0) as i64);
    metrics.projections.insert("YEAR".to_string(), (velocity * 365.0 * 86400.0) as i64);

    // Finalize charts
    let mut yearly: Vec<_> = year_map.into_iter().collect();
    yearly.sort_by_key(|x| x.0);
    metrics.yearly_distribution = yearly;

    if let Some(db) = bowie_db {
        let mut completion = Vec::new();
        for rg in db.release_groups.values() {
            let heard = album_unique_tracks.get(&rg.title).map(|s| s.len()).unwrap_or(0);
            if heard > 0 {
                let pct = (heard as f64 / rg.track_count as f64).min(1.0);
                completion.push((rg.title.clone(), pct, rg.image_url.clone()));
            }
        }
        completion.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        metrics.album_completion = completion.into_iter().take(10).collect();
    }

    metrics.hourly_activity = (0..24).map(|h| (h, *hour_map.get(&h).unwrap_or(&0))).collect();
    let mut types: Vec<_> = type_map.into_iter().collect();
    types.sort_by(|a, b| b.1.cmp(&a.1));
    metrics.type_distribution = types;
    let mut lead: Vec<_> = track_minutes.into_iter().collect();
    lead.sort_by(|a, b| b.1.cmp(&a.1));
    metrics.track_time_leaderboard = lead.into_iter().take(10).collect();
    metrics.discovery_timeline = discovery_points.into_iter().rev().take(20).rev().collect();

    let mut weights = Vec::new();
    for (title, count) in album_scrobbles {
        let art = bowie_db.and_then(|db| db.release_groups.values().find(|rg| rg.title == title).and_then(|rg| rg.image_url.clone()));
        weights.push((title, count, art));
    }
    weights.sort_by(|a, b| b.1.cmp(&a.1));
    metrics.album_weight = weights.into_iter().take(15).collect();

    // Finalize Forgotten
    let mut forgotten = Vec::new();
    for (name, ls) in &last_seen_map {
        let idle = (now_ts - ls) / 86400;
        let tot = *total_count_map.get(name).unwrap_or(&0);
        if idle >= 30 && tot > 2 { forgotten.push((name.clone(), idle, tot)); }
    }
    forgotten.sort_by(|a, b| b.2.cmp(&a.2));
    metrics.forgotten_classics = forgotten.into_iter().take(10).collect();

    let mut consistency = Vec::new();
    for i in (0..30).rev() {
        let ts = today_start_ts - (i * 86400);
        consistency.push((ts, day_aggregates.get(&ts).map(|w| w.scrobbles).unwrap_or(0)));
    }
    metrics.consistency_grid = consistency;

    let mut monthly_v = Vec::new();
    for i in (0..12).rev() {
        let m_dt = now - Duration::days(i * 30);
        let k = (m_dt.year(), m_dt.month());
        monthly_v.push((format_month(k.1).chars().take(3).collect(), monthly_wrapped_map.get(&k).map(|w| w.scrobbles).unwrap_or(0)));
    }
    metrics.monthly_volume = monthly_v;

    // Song of the day
    let mut s_stats: Vec<_> = total_count_map.into_iter().filter(|(n, _)| {
        let ls = last_seen_map.get(n).unwrap_or(&0);
        *ls < now_ts - (30 * 86400)
    }).collect();
    s_stats.sort_by(|a, b| b.1.cmp(&a.1));
    if let Some((n, _)) = s_stats.first() {
        let alb = bowie_db.and_then(|db| db.release_groups.values().find(|rg| rg.tracks.iter().any(|t| &t.title == n)).map(|rg| rg.title.clone())).unwrap_or_default();
        metrics.song_of_the_day = Some((n.clone(), alb));
    } else if let Some(db) = bowie_db {
        let rgs: Vec<_> = db.release_groups.values().collect();
        if !rgs.is_empty() {
            let s = now.format("%Y%m%d").to_string().parse::<usize>().unwrap_or(0);
            let rg = &rgs[s % rgs.len()];
            if !rg.tracks.is_empty() {
                metrics.song_of_the_day = Some((rg.tracks[s % rg.tracks.len()].title.clone(), rg.title.clone()));
            }
        }
    }

    metrics.last_listen_display = if let Some(l) = sorted_listens.last() { format_relative_time(l.listened_at, now_ts) } else { "Never".to_string() };
    metrics
}

fn get_bowie_album_tracks(name: &str, ex: &HashMap<String, usize>, db: Option<&BowieDatabase>) -> f64 {
    if let Some(d) = db {
        let n_l = name.to_lowercase();
        if let Some(rg) = d.release_groups.values().find(|r| r.title.to_lowercase() == n_l) { return rg.track_count as f64; }
    }
    if let Some(c) = ex.get(name) { return *c as f64; }
    11.0
}

fn calculate_total_completion(work: &DayWork, ex: &HashMap<String, usize>, db: Option<&BowieDatabase>) -> f64 {
    work.album_counts.iter().map(|(n, &c)| c as f64 / get_bowie_album_tracks(n, ex, db)).sum()
}

fn get_top_albums(counts: &HashMap<String, usize>, ms: &HashMap<String, i64>, n: usize, ex: &HashMap<String, usize>, db: Option<&BowieDatabase>) -> Vec<(String, f64, i64)> {
    let mut items: Vec<_> = counts.iter().map(|(name, &c)| (name.clone(), c as f64 / get_bowie_album_tracks(name, ex, db), *ms.get(name).unwrap_or(&0))).collect();
    items.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    items.into_iter().take(n).collect()
}

fn get_top_items(counts: &HashMap<String, usize>, ms: &HashMap<String, i64>, n: usize) -> Vec<(String, usize, i64)> {
    let mut items: Vec<_> = counts.iter().map(|(name, &c)| (name.clone(), c, *ms.get(name).unwrap_or(&0))).collect();
    items.sort_by(|a, b| b.1.cmp(&a.1));
    items.into_iter().take(n).collect()
}

pub fn is_bowie(l: &Listen, mbids: &HashSet<String>) -> bool {
    is_bowie_meta(&l.track_metadata, mbids)
}

pub fn is_bowie_meta(m: &crate::models::TrackMetadata, mbids: &HashSet<String>) -> bool {
    if !mbids.is_empty() {
        let id = m.mbid_mapping.as_ref().and_then(|x| x.recording_mbid.as_ref()).or_else(|| m.additional_info.as_ref().and_then(|i| i.recording_mbid.as_ref()));
        if let Some(i) = id { if mbids.contains(i) { return true; } }
    }
    let a_m = m.mbid_mapping.as_ref().and_then(|x| x.artists.as_ref()).and_then(|a| a.first()).map(|a| a.artist_credit_name.as_str());
    if let Some(a) = a_m { if a.to_lowercase().contains("bowie") { return true; } }
    m.artist_name.to_lowercase().contains("bowie")
}

pub fn format_relative_time(ts: i64, now: i64) -> String {
    let d = now - ts;
    if d < 60 { return "Just now".to_string(); }
    if d < 3600 { return format!("{}m ago", d / 60); }
    if d < 86400 { return format!("{}h ago", d / 3600); }
    Utc.timestamp_opt(ts, 0).unwrap().format("%b %d").to_string()
}

fn get_listening_day_range(now: DateTime<Utc>) -> (i64, i64) {
    let mut s = Utc.with_ymd_and_hms(now.year(), now.month(), now.day(), 5, 0, 0).unwrap();
    if now.hour() < 5 { s = s - Duration::days(1); }
    (s.timestamp(), (s + Duration::hours(21)).timestamp())
}

fn get_listening_day_start(ts: i64) -> i64 {
    let dt = Utc.timestamp_opt(ts, 0).unwrap();
    let mut s = Utc.with_ymd_and_hms(dt.year(), dt.month(), dt.day(), 5, 0, 0).unwrap();
    if dt.hour() < 5 { s = s - Duration::days(1); }
    s.timestamp()
}

fn format_month(m: u32) -> &'static str {
    match m { 1 => "January", 2 => "February", 3 => "March", 4 => "April", 5 => "May", 6 => "June", 7 => "July", 8 => "August", 9 => "September", 10 => "October", 11 => "November", 12 => "December", _ => "Unknown" }
}