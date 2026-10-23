use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

const PROGRESS_EVENT: &str = "update://progress";

static INSTALLING: AtomicBool = AtomicBool::new(false);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    current_version: String,
    available: bool,
    version: Option<String>,
    notes: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    downloaded: u64,
    total: Option<u64>,
}

struct InstallGuard;

impl Drop for InstallGuard {
    fn drop(&mut self) {
        INSTALLING.store(false, Ordering::SeqCst);
    }
}

pub async fn check(app: &AppHandle) -> Result<UpdateCheck, String> {
    let updater = app.updater().map_err(|e| format!("更新组件未就绪：{e}"))?;
    let update = updater
        .check()
        .await
        .map_err(|e| format!("检查更新失败：{e}"))?;

    let current_version = env!("CARGO_PKG_VERSION").to_string();
    match update {
        Some(update) => {
            tracing::info!(version = %update.version, "发现新版本");
            Ok(UpdateCheck {
                current_version,
                available: true,
                version: Some(update.version),
                notes: update.body,
            })
        }
        None => Ok(UpdateCheck {
            current_version,
            available: false,
            version: None,
            notes: None,
        }),
    }
}

pub async fn install(app: &AppHandle) -> Result<String, String> {
    if INSTALLING.swap(true, Ordering::SeqCst) {
        return Err("正在更新，请稍候".into());
    }
    let _guard = InstallGuard;

    let updater = app.updater().map_err(|e| format!("更新组件未就绪：{e}"))?;
    let Some(update) = updater
        .check()
        .await
        .map_err(|e| format!("检查更新失败：{e}"))?
    else {
        return Err("已是最新版本，无需更新".into());
    };

    let version = update.version.clone();
    tracing::info!(version = %version, "开始下载更新");

    let handle = app.clone();
    let mut downloaded = 0u64;
    let mut last_emit = std::time::Instant::now();
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let done = total.is_some_and(|t| downloaded >= t);
                if done || last_emit.elapsed().as_millis() >= 100 {
                    last_emit = std::time::Instant::now();
                    let _ = handle.emit(PROGRESS_EVENT, Progress { downloaded, total });
                }
            },
            || {},
        )
        .await
        .map_err(|e| format!("安装更新失败：{e}"))?;

    tracing::info!(version = %version, "更新已安装，重启后生效");
    Ok(version)
}
