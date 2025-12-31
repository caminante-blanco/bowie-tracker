use leptos::*;
use crate::analytics::DashboardMetrics;

#[component]
pub fn ListeningHistoryChart(metrics: Memo<DashboardMetrics>) -> impl IntoView {
    let m = metrics.get();

    view! {
        <div style="display: flex; flex-direction: column; gap: 30px; padding-bottom: 50px;">
            
            // 1. Consistency Grid (Heatmap style)
            <section class="card">
                <h3 class="stat-label">"Last 30 Days Activity"</h3>
                <div style="display: grid; grid-template-columns: repeat(10, 1fr); gap: 5px; margin-top: 10px;">
                    {m.consistency_grid.iter().map(|(_, count)| {
                        let opacity = (*count as f32 / 20.0).clamp(0.1, 1.0);
                        view! { <div style=format!("aspect-ratio: 1; background: var(--primary); border-radius: 2px; opacity: {};", opacity) title=format!("{} scrobbles", count)></div> }
                    }).collect_view()}
                </div>
            </section>

            // 2. Album Completion
            <section class="card">
                <h3 class="stat-label">"Album Completion (Unique Tracks)"</h3>
                <div style="display: flex; flex-direction: column; gap: 12px; margin-top: 15px;">
                    {m.album_completion.iter().map(|(title, pct, art)| {
                        view! {
                            <div style="display: flex; flex-direction: column; gap: 4px;">
                                <div style="display: flex; justify-content: space-between; font-size: 0.7rem;">
                                    <span>{title}</span>
                                    <span style="color: var(--primary); font-weight: bold;">{format!("{:.0}%", pct * 100.0)}</span>
                                </div>
                                <div style="height: 8px; background: var(--surface); border-radius: 4px; overflow: hidden;">
                                    <div style=format!("width: {}%; height: 100%; background: var(--primary);", pct * 100.0)></div>
                                </div>
                            </div>
                        }
                    }).collect_view()}
                </div>
            </section>

            <div class="grid-container" style="grid-template-columns: 1fr 1fr; gap: 20px;">
                // 3. Yearly Affinity
                <section class="card">
                    <h3 class="stat-label">"Scrobbles by Year"</h3>
                    <div style="display: flex; align-items: flex-end; gap: 4px; height: 100px; margin-top: 15px;">
                        {let max = m.yearly_distribution.iter().map(|x| x.1).max().unwrap_or(1).max(1);
                         m.yearly_distribution.iter().map(|(year, count)| {
                            let h = (*count as f64 / max as f64) * 100.0;
                            view! { <div style=format!("flex: 1; height: {}%; background: var(--secondary); border-radius: 2px 2px 0 0;", h) title=format!("{}: {}", year, count)></div> }
                        }).collect_view()}
                    </div>
                </section>

                // 4. Hourly Activity
                <section class="card">
                    <h3 class="stat-label">"24h Listening Pattern"</h3>
                    <div style="display: flex; align-items: flex-end; gap: 2px; height: 100px; margin-top: 15px;">
                        {let max = m.hourly_activity.iter().map(|x| x.1).max().unwrap_or(1).max(1);
                         m.hourly_activity.iter().map(|(h, count)| {
                            let ht = (*count as f64 / max as f64) * 100.0;
                            view! { <div style=format!("flex: 1; height: {}%; background: var(--accent); border-radius: 1px;", ht) title=format!("{}:00 - {} scrobbles", h, count)></div> }
                        }).collect_view()}
                    </div>
                </section>
            </div>

            // 5. Track Time Leaderboard
            <section class="card">
                <h3 class="stat-label">"Top Tracks by Total Time"</h3>
                <div style="display: flex; flex-direction: column; gap: 8px; margin-top: 15px;">
                    {m.track_time_leaderboard.iter().map(|(name, mins)| {
                        view! {
                            <div style="display: flex; justify-content: space-between; font-size: 0.8rem; border-bottom: 1px solid var(--surface); padding-bottom: 4px;">
                                <span style="white-space: nowrap; overflow: hidden; text-overflow: ellipsis; flex: 1;">{name}</span>
                                <span style="font-weight: 900; color: var(--accent);">{mins}"m"</span>
                            </div>
                        }
                    }).collect_view()}
                </div>
            </section>

            // 6. Release Type Distribution
            <section class="card">
                <h3 class="stat-label">"Discography Mix"</h3>
                <div style="display: flex; height: 30px; border-radius: 15px; overflow: hidden; margin-top: 15px;">
                    {let total: usize = m.type_distribution.iter().map(|x| x.1).sum();
                     let colors = ["var(--primary)", "var(--secondary)", "var(--accent)", "#98971a", "#8ec07c"];
                     m.type_distribution.iter().enumerate().map(|(i, (t, count))| {
                        let w = (*count as f64 / total as f64) * 100.0;
                        view! { <div style=format!("width: {}%; background: {}; height: 100%;", w, colors[i % colors.len()]) title=format!("{}: {}%", t, w as i32)></div> }
                    }).collect_view()}
                </div>
                <div style="display: flex; flex-wrap: wrap; gap: 10px; margin-top: 10px; font-size: 0.6rem; font-weight: bold;">
                    {m.type_distribution.iter().map(|(t, _)| view! { <span>{t.to_uppercase()}</span> }).collect_view()}
                </div>
            </section>

            // 7. Discovery Timeline (Recent)
            <section class="card">
                <h3 class="stat-label">"New Song Discovery Timeline"</h3>
                <div style="display: flex; align-items: flex-end; gap: 5px; height: 80px; margin-top: 15px;">
                    {let max = m.discovery_timeline.iter().map(|x| x.1).max().unwrap_or(1).max(1);
                     m.discovery_timeline.iter().map(|(_, count)| {
                        let h = (*count as f64 / max as f64) * 100.0;
                        view! { <div style=format!("flex: 1; height: {}%; background: #ebdbb2; border-radius: 2px;", h)></div> }
                    }).collect_view()}
                </div>
            </section>

            // 8. Album Weight (Relative Size)
            <section class="card">
                <h3 class="stat-label">"Most Played Albums"</h3>
                <div style="display: flex; flex-wrap: wrap; gap: 15px; margin-top: 15px; justify-content: center; align-items: center;">
                    {let max = m.album_weight.iter().map(|x| x.1).max().unwrap_or(1).max(1);
                     m.album_weight.iter().map(|(title, count, art)| {
                        let size = 40.0 + (*count as f64 / max as f64) * 60.0;
                        view! {
                            <div style="position: relative;" title=format!("{}: {} plays", title, count)>
                                {art.as_ref().map(|url| view! {
                                    <img src=url style=format!("width: {}px; height: {}px; border-radius: 4px; object-fit: cover; box-shadow: 0 4px 8px rgba(0,0,0,0.3);", size, size) alt=title.clone()/>
                                }).or_else(|| Some(view! {
                                    <div style=format!("width: {}px; height: {}px; background: var(--surface); border-radius: 4px;", size, size)></div>
                                }.into_view()))}
                            </div>
                        }
                    }).collect_view()}
                </div>
            </section>

            // 9. Monthly Volume
            <section class="card">
                <h3 class="stat-label">"Monthly Scrobbles (Last 12)"</h3>
                <div style="display: flex; align-items: flex-end; gap: 10px; height: 120px; margin-top: 15px; padding: 0 10px;">
                    {let max = m.monthly_volume.iter().map(|x| x.1).max().unwrap_or(1).max(1);
                     m.monthly_volume.iter().map(|(label, count)| {
                        let h = (*count as f64 / max as f64) * 100.0;
                        view! {
                            <div style="flex: 1; display: flex; flex-direction: column; height: 100%; justify-content: flex-end; align-items: center; gap: 5px;">
                                <div style=format!("width: 100%; height: {}%; background: var(--primary); border-radius: 4px 4px 0 0;", h)></div>
                                <span style="font-size: 0.5rem; font-weight: bold; color: #a89984;">{label}</span>
                            </div>
                        }
                    }).collect_view()}
                </div>
            </section>

            // 10. Forgotten Classics
            <section class="card">
                <h3 class="stat-label">"Classics Gathering Dust (>30 days idle)"</h3>
                <div style="display: flex; flex-direction: column; gap: 10px; margin-top: 15px;">
                    {m.forgotten_classics.iter().map(|(name, idle, total)| {
                        view! {
                            <div style="display: flex; justify-content: space-between; align-items: center; background: rgba(0,0,0,0.1); padding: 8px; border-radius: 4px;">
                                <div style="flex: 1;">
                                    <div style="font-size: 0.8rem; font-weight: bold;">{name}</div>
                                    <div style="font-size: 0.6rem; color: var(--accent);">"Total plays: "{total}</div>
                                </div>
                                <div style="text-align: right;">
                                    <div style="font-size: 0.7rem; font-weight: 900; color: #a89984;">{idle}" DAYS IDLE"</div>
                                </div>
                            </div>
                        }
                    }).collect_view()}
                </div>
            </section>

        </div>
    }
}