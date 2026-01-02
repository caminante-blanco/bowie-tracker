//! Analytics and Bowie-specific filtering logic for Ziggy.
use chrono::{DateTime, Utc, TimeZone, Datelike, Timelike, Duration};
use std::collections::{HashMap, HashSet};
use crate::models::{Listen, BowieLookup};
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
    basis: &str, 
    _external_counts: &HashMap<String, usize>,
    lookup: &BowieLookup
) -> DashboardMetrics {
    let mut metrics = DashboardMetrics::default();
    if lookup.recordings.is_empty() { return metrics; }

    // 1. Filter STRICTLY by MBID. No string fallbacks allowed.
    let bowie_listens: Vec<&Listen> = listens.iter()
        .filter(|l| is_bowie_meta(&l.track_metadata, lookup))
        .collect();

    if bowie_listens.is_empty() { return metrics; }

    // 2. Aggregation Loop
    let now_ts = now.timestamp();
    let today_start_ts = get_listening_day_start(now_ts);
    let mut day_aggregates: HashMap<i64, DayWork> = HashMap::new();
    let mut monthly_wrapped_map: HashMap<(i32, u32), DayWork> = HashMap::new();
    
    let mut year_distribution = HashMap::new();
    let mut hour_distribution = HashMap::new();
    let mut type_distribution = HashMap::new();
    let mut album_unique_tracks: HashMap<String, HashSet<String>> = HashMap::new();
    let mut track_time_map: HashMap<String, i64> = HashMap::new();
    let mut discovery_points = Vec::new();
    let mut unique_mbids_seen = HashSet::new();
    let mut total_scrobble_count: HashMap<String, usize> = HashMap::new();
    let mut last_seen_map: HashMap<String, i64> = HashMap::new();
    let mut rg_images: HashMap<String, String> = HashMap::new();
    let mut rg_max_tracks: HashMap<String, usize> = HashMap::new();

    for listen in &bowie_listens {
        let ts = listen.listened_at;
        let dt = Utc.timestamp_opt(ts, 0).unwrap();
        let day_ts = get_listening_day_start(ts);
        
        let mbid = listen.track_metadata.mbid_mapping.as_ref()
            .and_then(|m| m.recording_mbid.as_ref())
            .or_else(|| listen.track_metadata.additional_info.as_ref().and_then(|i| i.recording_mbid.as_ref()))
            .unwrap(); // Filtered above, safe to unwrap

        let rg_id = lookup.recordings.get(mbid).unwrap();
        let (rg_title, art, rg_count, rg_type) = lookup.release_groups.get(rg_id).unwrap();
        
        if let Some(url) = art { rg_images.insert(rg_title.clone(), url.clone()); }
        let entry_count = rg_max_tracks.entry(rg_title.clone()).or_insert(0);
        if *rg_count > *entry_count { *entry_count = *rg_count; }

        let dur_ms = listen.track_metadata.additional_info.as_ref().and_then(|i| i.duration_ms).unwrap_or(210_000);
        let min = dur_ms / 60000;

        let d_work = day_aggregates.entry(day_ts).or_insert_with(DayWork::default);
        let m_work = monthly_wrapped_map.entry((dt.year(), dt.month())).or_insert_with(DayWork::default);

        for work in [d_work, m_work] {
            work.ms += dur_ms;
            work.scrobbles += 1;
            *work.album_counts.entry(rg_title.clone()).or_insert(0) += 1;
            *work.track_counts.entry(listen.track_metadata.track_name.clone()).or_insert(0) += 1;
            *work.album_ms.entry(rg_title.clone()).or_insert(0) += min;
            *work.track_ms.entry(listen.track_metadata.track_name.clone()).or_insert(0) += min;
        }

        if ts >= today_start_ts { metrics.counts.today += 1; metrics.minutes.today += min; }
        if ts >= now_ts - (7 * 86400) { metrics.counts.week += 1; metrics.minutes.week += min; }
        if ts >= now_ts - (30 * 86400) { metrics.counts.month += 1; metrics.minutes.month += min; }
        metrics.counts.total += 1;
        metrics.minutes.total += min;

        *year_distribution.entry(dt.year()).or_insert(0) += 1;
        *hour_distribution.entry(dt.hour()).or_insert(0) += 1;
        if let Some(t) = rg_type { *type_distribution.entry(t.clone()).or_insert(0) += 1; }

        album_unique_tracks.entry(rg_title.clone()).or_default().insert(mbid.clone());
        *track_time_map.entry(listen.track_metadata.track_name.clone()).or_insert(0) += min;
        if unique_mbids_seen.insert(mbid.clone()) { discovery_points.push((ts, unique_mbids_seen.len())); }
        *total_scrobble_count.entry(listen.track_metadata.track_name.clone()).or_insert(0) += 1;
        last_seen_map.insert(listen.track_metadata.track_name.clone(), ts);
    }

    // 3. Finalize Stats
    let mut day_stats_map: HashMap<i64, DayStats> = HashMap::new();
    for (day_ts, work) in &day_aggregates {
        let mut stats = DayStats {
            timestamp: *day_ts,
            date_label: Utc.timestamp_opt(*day_ts, 0).unwrap().format("%a, %b %d").to_string(),
            minutes: work.ms / 60000,
            scrobbles: work.scrobbles,
            ..Default::default()
        };
        stats.albums_completed = work.album_counts.iter().map(|(title, _)| {
            let seen = album_unique_tracks.get(title).map(|s| s.len()).unwrap_or(0);
            let total = *rg_max_tracks.get(title).unwrap_or(&11);
            seen as f64 / total as f64
        }).sum();

        let mut top_albs: Vec<_> = work.album_counts.iter().map(|(title, _)| {
            let seen = album_unique_tracks.get(title).map(|s| s.len()).unwrap_or(0);
            let total = *rg_max_tracks.get(title).unwrap_or(&11);
            (title.clone(), seen as f64 / total as f64, *work.album_ms.get(title).unwrap_or(&0))
        }).collect();
        top_albs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        stats.top_albums = top_albs.into_iter().take(5).collect();

        let mut top_trks: Vec<_> = work.track_counts.iter().map(|(n, c)| (n.clone(), *c, *work.track_ms.get(n).unwrap_or(&0))).collect();
        top_trks.sort_by_key(|x| std::cmp::Reverse(x.1));
        stats.top_tracks = top_trks.into_iter().take(5).collect();
        stats.favorite_album = stats.top_albums.first().map(|a| a.0.clone()).unwrap_or_default();
        day_stats_map.insert(*day_ts, stats);
    }

    let mut history: Vec<DayStats> = day_stats_map.values().cloned().collect();
    history.sort_by_key(|d| std::cmp::Reverse(d.timestamp));
    
    let mut sorted_indices: Vec<usize> = (0..history.len()).collect();
    sorted_indices.sort_by_key(|&i| std::cmp::Reverse(history[i].minutes));
    for (rank, &idx) in sorted_indices.iter().enumerate().take(2) {
        history[idx].badge = Some(if rank == 0 { "PEAK SESSION".into() } else { "HIGH ACTIVITY".into() });
    }
    metrics.history = history;

    for ((y, m), work) in &monthly_wrapped_map {
        let mut wrapped = MonthlyWrapped { year: *y, month: *m, month_name: format_month(*m).into(), total_scrobbles: work.scrobbles, total_minutes: work.ms / 60000, ..Default::default() };
        let mut m_days: Vec<_> = metrics.history.iter().filter(|d| { let dt = Utc.timestamp_opt(d.timestamp, 0).unwrap(); dt.year() == *y && dt.month() == *m }).cloned().collect();
        m_days.sort_by_key(|d| std::cmp::Reverse(d.timestamp));
        wrapped.days = m_days;
        metrics.rewards.push(wrapped);
    }
    metrics.rewards.sort_by(|a, b| b.year.cmp(&a.year).then(b.month.cmp(&a.month)));

    let mut reward_indices: Vec<usize> = (0..metrics.rewards.len()).collect();
    reward_indices.sort_by_key(|&i| std::cmp::Reverse(metrics.rewards[i].total_minutes));
    for (rank, &idx) in reward_indices.iter().enumerate().take(2) {
        metrics.rewards[idx].badge = Some(if rank == 0 { "MILESTONE MONTH".into() } else { "TOP PERIOD".into() });
    }

    metrics.yearly_distribution = year_distribution.into_iter().collect();
    metrics.yearly_distribution.sort_by_key(|x| x.0);
    metrics.hourly_activity = (0..24).map(|h| (h, *hour_distribution.get(&h).unwrap_or(&0))).collect();
    metrics.type_distribution = type_distribution.into_iter().collect();
    metrics.type_distribution.sort_by_key(|x| std::cmp::Reverse(x.1));
    metrics.track_time_leaderboard = track_time_map.into_iter().collect();
    metrics.track_time_leaderboard.sort_by_key(|x| std::cmp::Reverse(x.1));
    metrics.track_time_leaderboard.truncate(10);
    metrics.discovery_timeline = discovery_points.into_iter().rev().take(20).rev().collect();
    
    metrics.album_completion = album_unique_tracks.iter().map(|(title, seen)| {
        let total = *rg_max_tracks.get(title).unwrap_or(&11);
        (title.clone(), seen.len() as f64 / total as f64, rg_images.get(title).cloned())
    }).collect();
    metrics.album_completion.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    metrics.albums.today = metrics.counts.today as f64 / 11.0;
    metrics.albums.total = metrics.counts.total as f64 / 11.0;

    let d_elapsed = (now_ts - today_start_ts).max(1) as f64;
    let d_vel = metrics.minutes.today as f64 / d_elapsed;
    let w_vel = metrics.minutes.week as f64 / (7.0 * 86400.0);
    let m_vel = metrics.minutes.month as f64 / (30.0 * 86400.0);
    let y_vel = metrics.minutes.total as f64 / (now_ts - bowie_listens[0].listened_at).max(1) as f64;
    
    let vel = match basis {
        "DAY" => d_vel,
        "WEEK" => w_vel,
        "MONTH" => m_vel,
        "YEAR" => y_vel,
        _ => w_vel
    };

    metrics.projections.insert("DAY".into(), (vel * 86400.0) as i64);
    metrics.projections.insert("WEEK".into(), (vel * 604800.0) as i64);
    metrics.projections.insert("MONTH".into(), (vel * 2592000.0) as i64);
    metrics.projections.insert("YEAR".into(), (vel * 31536000.0) as i64);

    let mut s_stats: Vec<(String, usize)> = total_scrobble_count.into_iter().filter(|(n, _)| {
        let ls = last_seen_map.get(n).unwrap_or(&0);
        *ls < now_ts - (30 * 86400)
    }).collect();
    s_stats.sort_by_key(|x| std::cmp::Reverse(x.1));
    
    if let Some((n, _)) = s_stats.first() {
        let rg_title = lookup.recordings.iter().find_map(|(rec_id, rg_id)| {
            let (title, _, _, _) = lookup.release_groups.get(rg_id).unwrap();
            if rec_id == rec_id { Some(title.clone()) } else { None }
        }).unwrap_or_default();
        metrics.song_of_the_day = Some((n.clone(), rg_title));
    }

    metrics.last_listen_display = format_relative_time(bowie_listens.last().unwrap().listened_at, now_ts);
    metrics
}

pub fn is_bowie_meta(m: &crate::models::TrackMetadata, lookup: &BowieLookup) -> bool {
    let id = m.mbid_mapping.as_ref().and_then(|x| x.recording_mbid.as_ref()).or_else(|| m.additional_info.as_ref().and_then(|i| i.recording_mbid.as_ref()));
    id.map(|i| lookup.recordings.contains_key(i)).unwrap_or(false)
}

pub fn match_playing_now(
    m: &crate::models::TrackMetadata, 
    lookup: &BowieLookup,
    last_rg_mbid: Option<&String>
) -> Option<(String, String)> { // (Recording MBID, Release Group MBID)
    // 1. Direct MBID match
    let direct_id = m.mbid_mapping.as_ref()
        .and_then(|x| x.recording_mbid.as_ref())
        .or_else(|| m.additional_info.as_ref().and_then(|i| i.recording_mbid.as_ref()));

    if let Some(id) = direct_id {
        if let Some(rg_id) = lookup.recordings.get(id) {
            return Some((id.clone(), rg_id.clone()));
        }
    }

    // 2. Name-based match fallback
    let norm_name = m.track_name.to_lowercase();
    if let Some(matches) = lookup.name_map.get(&norm_name) {
        if matches.is_empty() { return None; }
        
        // If we have a last seen album, prioritize a match from that album
        if let Some(last_rg) = last_rg_mbid {
            if let Some(preferred) = matches.iter().find(|(_, rg)| rg == last_rg) {
                return Some(preferred.clone());
            }
        }
        
        // Otherwise, just take the first match
        return Some(matches[0].clone());
    }
    
    None
}

pub fn format_relative_time(ts: i64, now: i64) -> String {
    let d = now - ts;
    if d < 60 { return "Just now".into(); }
    if d < 3600 { return format!("{}m ago", d / 60); }
    if d < 86400 { return format!("{}h ago", d / 3600); }
    Utc.timestamp_opt(ts, 0).unwrap().format("%b %d").to_string()
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
