use serde_json::Value;
use tauri::AppHandle;
use tauri_plugin_aptabase::EventTracker;

pub const APP_KEY: &str = "A-US-2179981546";

pub fn track(app: &AppHandle, event: &str, props: Option<Value>) {
    if !crate::prefs::load(app).analytics_enabled {
        return;
    }
    if let Err(e) = app.track_event(event, props) {
        tracing::debug!(event, "统计事件入队失败：{e}");
    }
}

pub fn flush(app: &AppHandle) {
    app.flush_events_blocking();
}

pub fn duration_bucket(ms: u64) -> &'static str {
    match ms {
        0..=9_999 => "<10s",
        10_000..=59_999 => "<1m",
        60_000..=299_999 => "<5m",
        300_000..=899_999 => "<15m",
        900_000..=3_599_999 => "<60m",
        _ => ">=60m",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_buckets_cover_boundaries() {
        assert_eq!(duration_bucket(0), "<10s");
        assert_eq!(duration_bucket(9_999), "<10s");
        assert_eq!(duration_bucket(10_000), "<1m");
        assert_eq!(duration_bucket(59_999), "<1m");
        assert_eq!(duration_bucket(60_000), "<5m");
        assert_eq!(duration_bucket(3_599_999), "<60m");
        assert_eq!(duration_bucket(3_600_000), ">=60m");
    }
}
