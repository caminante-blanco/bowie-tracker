//! Analytics and Bowie-specific filtering logic for Ziggy.
//! 
//! MIT License
//! 
//! Copyright (c) 2024 RustyNova (Alistral Philosophy)
//! 
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the "Software"), to deal
//! in the Software without restriction, including without limitation the rights
//! to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//! copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//! 
//! The above copyright notice and this permission notice shall be included in all
//! copies or substantial portions of the Software.
//! 
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//! SOFTWARE.

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
    pub yearly_distribution: Vec<(i32, usize)>, // Year -> Scrobble Count
    pub album_completion: Vec<(String, f64, Option<String>)>, // Title, %, Image
    pub monthly_volume: Vec<(String, usize)>, // Label -> Count
    pub track_time_leaderboard: Vec<(String, i64)>, // Track -> Minutes
    pub hourly_activity: Vec<(u32, usize)>, // Hour (0-23) -> Count
    pub type_distribution: Vec<(String, usize)>, // Type -> Count
    pub discovery_timeline: Vec<(i64, usize)>, // TS -> Cumulative Unique MBIDs
    pub consistency_grid: Vec<(i64, usize)>, // Last 30 days TS -> Count
    pub album_weight: Vec<(String, usize, Option<String>)>, // Title, Count, Image
    pub forgotten_classics: Vec<(String, i64, usize)>, // Title, Days Idle, Total Count
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

pub fn calculate_metrics(listens: &[Listen], now: DateTime<Utc>, basis: &str, external_counts: &HashMap<String, usize>, bowie_db: Option<&BowieDatabase>) -> DashboardMetrics {
    // Pre-calculate bowie MBID, Title, and Duration maps for fast lookup
    let mut bowie_mbids = HashSet::new();
    let mut bowie_durations = HashMap::new();
    let mut bowie_title_durations = HashMap::new();

    if let Some(db) = bowie_db {
        for rg in db.release_groups.values() {
            for track in &rg.tracks {
                bowie_mbids.insert(track.id.clone());
                
                // Index by MBID
                let m_entry = bowie_durations.entry(track.id.clone()).or_insert(0);
                if track.duration_ms > *m_entry { *m_entry = track.duration_ms; }
                
                // Index by Literal Title
                let t_entry = bowie_title_durations.entry(track.title.clone()).or_insert(0);
                if track.duration_ms > *t_entry { *t_entry = track.duration_ms; }
            }
        }
    }

    // ... filtering ...

    for listen in &bowie_listens {
        // ... aggregation ...

        let mbid = listen.track_metadata.mbid_mapping.as_ref()
            .and_then(|m| m.recording_mbid.as_ref())
            .or_else(|| listen.track_metadata.additional_info.as_ref().and_then(|i| i.recording_mbid.as_ref()));

        let track_name = listen.track_metadata.mbid_mapping.as_ref()
            .and_then(|m| m.recording_name.clone())
            .unwrap_or_else(|| listen.track_metadata.track_name.clone());

        let duration_ms = mbid.and_then(|id| bowie_durations.get(id).cloned())
            .or_else(|| bowie_title_durations.get(&track_name).cloned())
            .unwrap_or(0); 
        
        let album_name = listen.track_metadata.mbid_mapping.as_ref()
            .and_then(|m| m.release_name.clone())
            .or_else(|| listen.track_metadata.release_name.clone())
            .unwrap_or_else(|| "Unknown Album".to_string());
        
        let track_name = listen.track_metadata.mbid_mapping.as_ref()
            .and_then(|m| m.recording_name.clone())
            .unwrap_or_else(|| listen.track_metadata.track_name.clone());

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
        // ... (wrapped calc)
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
        let mut sorted_wrapped = metrics.rewards.clone();
        sorted_wrapped.sort_by(|a, b| b.total_minutes.cmp(&a.total_minutes));
        metrics.insights.push(Insight {
            title: "2nd Most Active Month".to_string(),
            value: format!("{} {}", sorted_wrapped[1].month_name, sorted_wrapped[1].year),
            description: format!("Time: {}h", sorted_wrapped[1].total_minutes / 60),
        });
    }

    let day_elapsed = (now_ts - today_start_ts).max(1) as f64;
    let total_day_secs = 21.0 * 3600.0;
    let day_prog = (day_elapsed / total_day_secs).clamp(0.01, 1.0);

    let velocity = match basis {
        "DAY" => (metrics.minutes.today as f64 / day_prog) / total_day_secs,
        "WEEK" => (metrics.minutes.week as f64) / (7.0 * 86400.0),
        "MONTH" => (metrics.minutes.month as f64) / (30.0 * 86400.0),
        "YEAR" => (metrics.minutes.year as f64) / (365.0 * 86400.0),
        _ => 0.0
    };

    // --- START CHART CALCULATIONS ---
    
    // 1. Yearly Distribution
    let mut year_map = HashMap::new();
    // 2. Album Completion
    let mut album_unique_tracks: HashMap<String, HashSet<String>> = HashMap::new();
    // 4. Track Time Leaderboard
    let mut track_minutes: HashMap<String, i64> = HashMap::new();
    // 5. Hourly Activity
    let mut hour_map = HashMap::new();
    // 6. Type Distribution
    let mut type_map = HashMap::new();
    // 7. Discovery Timeline
    let mut unique_mbids_seen = HashSet::new();
    let mut discovery_points = Vec::new();
    // 9. Album Weight
    let mut album_scrobbles = HashMap::new();
    // 10. Forgotten Classics
    let mut last_seen_map: HashMap<String, i64> = HashMap::new();
    let mut total_count_map: HashMap<String, usize> = HashMap::new();

    // Iterate oldest to newest for Discovery Timeline
    let mut chron_listens = bowie_listens.clone();
    chron_listens.sort_by(|a, b| a.listened_at.cmp(&b.listened_at));

    for listen in &chron_listens {
        let ts = listen.listened_at;
        let dt = Utc.timestamp_opt(ts, 0).unwrap();
        let track_name = listen.track_metadata.track_name.clone();
        let album_name = listen.track_metadata.mbid_mapping.as_ref()
            .and_then(|m| m.release_name.clone())
            .or_else(|| listen.track_metadata.release_name.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        let mbid = listen.track_metadata.mbid_mapping.as_ref()
            .and_then(|m| m.recording_mbid.as_ref())
            .or_else(|| listen.track_metadata.additional_info.as_ref().and_then(|i| i.recording_mbid.as_ref()));

        // Discovery
        if let Some(id) = mbid {
            if unique_mbids_seen.insert(id.clone()) {
                discovery_points.push((ts, unique_mbids_seen.len()));
            }
        }

        // Duration for leaderboard
        let dur = mbid.and_then(|id| bowie_durations.get(id).cloned()).unwrap_or(0);
        *track_minutes.entry(track_name.clone()).or_insert(0) += dur / 60000;

        // Hourly
        *hour_map.entry(dt.hour()).or_insert(0) += 1;

        // Album stats
        *album_scrobbles.entry(album_name.clone()).or_insert(0) += 1;
        if let Some(id) = mbid {
            album_unique_tracks.entry(album_name.clone()).or_insert_with(HashSet::new).insert(id.clone());
        }

        // Year/Type via bowie_db
        if let Some(db) = bowie_db {
            // Find which RG this track/album belongs to
            let mut found_year = None;
            let mut found_type = None;
            for rg in db.release_groups.values() {
                if rg.title == album_name || rg.tracks.iter().any(|t| Some(&t.id) == mbid) {
                    // Try to extract year from first-release-date if we had it, 
                    // but for now we only have RG title/type.
                    // Let's assume we'll use the scrobble year as a proxy for "affinity year" 
                    // unless we improve the metadata later.
                    found_type = rg.release_type.clone();
                    break;
                }
            }
            if let Some(t) = found_type { *type_map.entry(t).or_insert(0) += 1; }
        }
        *year_map.entry(dt.year()).or_insert(0) += 1;

        // Forgotten Classics
        *total_count_map.entry(track_name.clone()).or_insert(0) += 1;
        let entry = last_seen_map.entry(track_name.clone()).or_insert(0);
        if ts > *entry { *entry = ts; }

fn get_bowie_album_tracks(name: &str, external_counts: &HashMap<String, usize>, bowie_db: Option<&BowieDatabase>) -> f64 {
    // 1. Check MusicBrainz metadata
    if let Some(db) = bowie_db {
        let name_low = name.to_lowercase();
        for rg in db.release_groups.values() {
            if rg.title.to_lowercase() == name_low {
                return rg.track_count as f64;
            }
        }
    }

    let n = name.to_lowercase();
    
    // 2. Check external counts (from IndexedDB/MB API)
    if let Some(count) = external_counts.get(name) {
        return *count as f64;
    }

    // 3. Comprehensive Hardcoded List
    if n.contains("david bowie") || n.contains("space oddity") { 10.0 }
    else if n.contains("man who sold the world") { 9.0 }
    else if n.contains("hunky dory") { 11.0 }
    else if n.contains("ziggy stardust") { 11.0 }
    else if n.contains("aladdin sane") { 10.0 }
    else if n.contains("pin ups") { 12.0 }
    else if n.contains("diamond dogs") { 11.0 }
    else if n.contains("young americans") { 8.0 }
    else if n.contains("station to station") { 6.0 }
    else if n.contains("low") { 11.0 }
    else if n.contains("heroes") { 10.0 }
    else if n.contains("lodger") { 10.0 }
    else if n.contains("scary monsters") { 10.0 }
    else if n.contains("let's dance") { 8.0 }
    else if n.contains("tonight") { 9.0 }
    else if n.contains("never let me down") { 10.0 }
    else if n.contains("black tie white noise") { 12.0 }
    else if n.contains("the buddha of suburbia") { 10.0 }
    else if n.contains("outside") { 19.0 }
    else if n.contains("earthling") { 9.0 }
    else if n.contains("hours") { 10.0 }
    else if n.contains("heathen") { 12.0 }
    else if n.contains("reality") { 11.0 }
    else if n.contains("the next day") { 14.0 }
    else if n.contains("blackstar") { 7.0 }
    else if n.contains("toy") { 12.0 }
    else if n.contains("david live") { 17.0 }
    else if n.contains("stage") { 17.0 }
    else if n.contains("the motion picture") { 15.0 }
    else { 11.0 } // Safe fallback
}

fn calculate_total_completion(work: &DayWork, external_counts: &HashMap<String, usize>, bowie_db: Option<&BowieDatabase>) -> f64 {
    let mut total = 0.0;
    for (name, count) in &work.album_counts {
        total += *count as f64 / get_bowie_album_tracks(name, external_counts, bowie_db);
    }
    total
}

fn get_top_albums(counts: &HashMap<String, usize>, mins: &HashMap<String, i64>, n: usize, external_counts: &HashMap<String, usize>, bowie_db: Option<&BowieDatabase>) -> Vec<(String, f64, i64)> {
    let mut items: Vec<_> = counts.iter().map(|(name, &c)| {
        let completion = c as f64 / get_bowie_album_tracks(name, external_counts, bowie_db);
        (name.clone(), completion, *mins.get(name).unwrap_or(&0))
    }).collect();
    items.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    items.into_iter().take(n).collect()
}

fn get_top_items(counts: &HashMap<String, usize>, mins: &HashMap<String, i64>, n: usize) -> Vec<(String, usize, i64)> {
    let mut items: Vec<_> = counts.iter().map(|(name, &c)| (name.clone(), c, *mins.get(name).unwrap_or(&0))).collect();
    items.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.cmp(&a.2)));
    items.into_iter().take(n).collect()
}

pub fn is_bowie(listen: &Listen, bowie_mbids: &HashSet<String>) -> bool {
    is_bowie_meta(&listen.track_metadata, bowie_mbids)
}

pub fn is_bowie_meta(meta: &crate::models::TrackMetadata, bowie_mbids: &HashSet<String>) -> bool {
    // 1. Check MBIDs if available
    if !bowie_mbids.is_empty() {
        let mbid = meta.mbid_mapping.as_ref()
            .and_then(|m| m.recording_mbid.as_ref())
            .or_else(|| meta.additional_info.as_ref().and_then(|i| i.recording_mbid.as_ref()));
        
        if let Some(id) = mbid {
            if bowie_mbids.contains(id) {
                return true;
            }
        }
    }

    // 2. Fallback to string matching
    let mapped_artist = meta.mbid_mapping
        .as_ref()
        .and_then(|m| m.artists.as_ref())
        .and_then(|a| a.first())
        .map(|a| a.artist_credit_name.as_str());

    if let Some(artist) = mapped_artist {
        if artist.to_lowercase().contains("bowie") { return true; }
    }
    meta.artist_name.to_lowercase().contains("bowie")
}

pub fn format_relative_time(ts: i64, now: i64) -> String {
    let diff = now - ts;
    if diff < 60 { return "Just now".to_string(); }
    if diff < 3600 { return format!("{}m ago", diff / 60); }
    if diff < 86400 { return format!("{}h ago", diff / 3600); }
    let dt = Utc.timestamp_opt(ts, 0).unwrap();
    dt.format("%b %d").to_string()
}

fn get_listening_day_range(now: DateTime<Utc>) -> (i64, i64) {
    let mut start = Utc.with_ymd_and_hms(now.year(), now.month(), now.day(), 5, 0, 0).unwrap();
    if now.hour() < 5 { start = start - Duration::days(1); }
    let end = start + Duration::hours(21);
    (start.timestamp(), end.timestamp())
}

fn get_listening_day_start(ts: i64) -> i64 {
    let dt = Utc.timestamp_opt(ts, 0).unwrap();
    let mut start = Utc.with_ymd_and_hms(dt.year(), dt.month(), dt.day(), 5, 0, 0).unwrap();
    if dt.hour() < 5 { start = start - Duration::days(1); }
    start.timestamp()
}

fn format_month(m: u32) -> &'static str {
    match m { 1 => "January", 2 => "February", 3 => "March", 4 => "April", 5 => "May", 6 => "June", 7 => "July", 8 => "August", 9 => "September", 10 => "October", 11 => "November", 12 => "December", _ => "Unknown" }
}
