use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;

use crate::config::{
    target_lang_for, EndpointConfig, EndpointProfile, OverlayConfig, OverlayMode, Preferences,
    PrefsPatch, SessionConfig, TranslateEngine,
};
use crate::{audio, overlay, paths, permission, prefs, session, shortcut};

#[derive(Debug, thiserror::Error)]
pub enum CmdError {
    #[error("未知命令：{0}")]
    Unknown(String),

    #[error("参数无效：{0}")]
    BadArgs(String),

    #[error("{message}")]
    Permission {
        permission: &'static str,
        message: String,
    },

    #[error("{0}")]
    Failed(String),
}

impl Serialize for CmdError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Permission {
                permission,
                message,
            } => json!({
                "kind": "permission",
                "permission": permission,
                "message": message,
            })
            .serialize(s),
            other => s.serialize_str(&other.to_string()),
        }
    }
}

#[tauri::command]
pub async fn dispatch(app: AppHandle, name: String, args: Value) -> Result<Value, CmdError> {
    let out = route_async(&app, &name, &args).await;

    if let Err(e) = &out {
        tracing::warn!(cmd = %name, error = %e, "命令失败");
    }

    out
}

pub async fn route_async(app: &AppHandle, name: &str, args: &Value) -> Result<Value, CmdError> {
    if matches!(
        name,
        "permission_request_mic"
            | "permission_probe_system_audio"
            | "session_start"
            | "session_stop"
    ) {
        let (app, name, args) = (app.clone(), name.to_string(), args.clone());
        return tauri::async_runtime::spawn_blocking(move || route(&app, &name, &args))
            .await
            .map_err(|e| CmdError::Failed(format!("后台任务失败：{e}")))?;
    }

    route(app, name, args)
}

pub fn route(app: &AppHandle, name: &str, args: &Value) -> Result<Value, CmdError> {
    match name {
        "app_info" => Ok(app_info()),
        "overlay_toggle" => overlay::toggle(app)
            .map(|v| json!(v))
            .map_err(CmdError::Failed),
        "overlay_set_mode" => {
            let raw = args
                .get("mode")
                .and_then(Value::as_str)
                .ok_or_else(|| CmdError::BadArgs("缺少 mode".into()))?;
            let mode = OverlayMode::parse(raw)
                .ok_or_else(|| CmdError::BadArgs(format!("未知模式 {raw}")))?;
            overlay::set_mode(app, mode)
                .map(|m| json!(m.as_str()))
                .map_err(CmdError::Failed)
        }

        "overlay_debug" => overlay::debug_state(app).map_err(CmdError::Failed),

        "prefs_get" => Ok(json!(prefs::load(app))),

        "prefs_set" => {
            let patch: PrefsPatch = args
                .get("patch")
                .map(|v| serde_json::from_value(v.clone()))
                .transpose()
                .map_err(|e| CmdError::BadArgs(format!("patch 解析失败：{e}")))?
                .ok_or_else(|| CmdError::BadArgs("缺少 patch".into()))?;

            patch.validate().map_err(CmdError::BadArgs)?;

            prefs::update(app, |p| patch.apply_to(p))
                .map(|p| json!(p))
                .map_err(CmdError::Failed)
        }

        "prefs_reset" => {
            let fresh = Preferences::default();
            prefs::update(app, |p| *p = fresh.clone())
                .map(|_| json!(fresh))
                .map_err(CmdError::Failed)
        }

        "permission_status" => Ok(json!(permission::snapshot())),

        "permission_request_mic" => permission::request_mic()
            .map(|r| json!(r))
            .map_err(CmdError::Failed),

        "permission_probe_system_audio" => {
            if app.state::<session::SessionState>().is_running() {
                return Err(CmdError::Failed("正在转写中，请先停止再检测".into()));
            }
            Ok(json!(permission::probe_system_audio()))
        }

        "open_privacy_settings" => {
            let pane = args
                .get("pane")
                .and_then(Value::as_str)
                .unwrap_or("systemAudio");
            let url = permission::settings_url(pane)
                .ok_or_else(|| CmdError::BadArgs(format!("未知设置页 {pane}")))?;
            #[cfg(target_os = "macos")]
            if let Err(e) = std::process::Command::new("open").arg(url).spawn() {
                tracing::warn!(pane, "无法直接打开系统设置页（URI scheme 可能已变更）：{e}");
            }
            Ok(json!(null))
        }

        "open_repo" => {
            open_with_system(std::ffi::OsStr::new(REPO_URL), "浏览器")?;
            Ok(json!(null))
        }

        "reveal_path" => {
            let target = args
                .get("path")
                .and_then(Value::as_str)
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| app.state::<crate::log::LogDir>().0.clone());
            reveal_in_file_manager(&target)?;
            Ok(json!(null))
        }

        "logs_export" => {
            let dir = app.state::<crate::log::LogDir>().0.clone();
            let dest = export_logs(app, &dir)?;
            Ok(json!({ "path": dest.display().to_string() }))
        }

        "transcript_export" => {
            let stamp = args
                .get("stamp")
                .and_then(Value::as_str)
                .unwrap_or("export");
            let path = export_transcript(app, stamp)?;
            Ok(json!({ "path": path.display().to_string() }))
        }

        "autostart_get" => Ok(json!({
            "enabled": app.autolaunch().is_enabled().unwrap_or(false),
        })),

        "autostart_set" => {
            let on = args
                .get("enabled")
                .and_then(Value::as_bool)
                .ok_or_else(|| CmdError::BadArgs("缺少 enabled".into()))?;

            let r = if on {
                app.autolaunch().enable()
            } else {
                app.autolaunch().disable()
            };
            r.map_err(|e| CmdError::Failed(format!("设置开机自启失败：{e}")))?;

            Ok(json!({
                "enabled": app.autolaunch().is_enabled().unwrap_or(false),
            }))
        }

        "model_info" => Ok(model_info(app)),

        "config_defaults" => Ok(json!({
            "overlay": OverlayConfig::default(),
            "endpoint": EndpointConfig::default(),
            "endpointProfiles": {
                "interpretation": EndpointConfig::for_profile(EndpointProfile::Interpretation),
                "dictation": EndpointConfig::for_profile(EndpointProfile::Dictation),
            },
        })),

        "overlay_config_get" => Ok(json!(overlay::config(app))),

        "overlay_visible" => Ok(json!(overlay::is_visible(app))),

        "overlay_config_set" => {
            let cfg: OverlayConfig = args
                .get("config")
                .map(|v| serde_json::from_value(v.clone()))
                .transpose()
                .map_err(|e| CmdError::BadArgs(format!("config 解析失败：{e}")))?
                .ok_or_else(|| CmdError::BadArgs("缺少 config".into()))?;

            let applied = overlay::set_config(app, cfg).map_err(CmdError::Failed)?;

            let ov = applied.clone();
            if let Err(e) = prefs::update(app, |p| p.overlay = ov) {
                tracing::error!("悬浮窗配置保存失败，本次调整不会被记住：{e}");
            }

            Ok(json!(applied))
        }

        "audio_devices" => {
            let source = args
                .get("source")
                .and_then(Value::as_str)
                .and_then(parse_source)
                .unwrap_or_default();
            audio::list_devices(source)
                .map(|d| json!(d))
                .map_err(|e| CmdError::Failed(e.to_string()))
        }

        "session_start" => {
            let cfg: SessionConfig = args
                .get("config")
                .map(|v| serde_json::from_value(v.clone()))
                .transpose()
                .map_err(|e| CmdError::BadArgs(format!("config 解析失败：{e}")))?
                .unwrap_or_default();

            let device = args
                .get("device")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .filter(|s| !s.is_empty());

            let source = args
                .get("source")
                .and_then(Value::as_str)
                .and_then(parse_source)
                .unwrap_or_default();

            let mut cfg = cfg;
            cfg.asr.endpoint = prefs::load(app).endpoint.sanitized();

            tracing::info!(
                source_lang = %cfg.translate.source_lang,
                target_lang = %target_lang_for(&cfg.translate.source_lang),
                engine = cfg.translate.engine.as_str(),
                trailing_silence_ms = cfg.asr.endpoint.trailing_silence_ms,
                max_utterance_ms = cfg.asr.endpoint.max_utterance_ms,
                "会话配置已解析（缺省字段已用默认值补齐）"
            );

            let device_for_prefs = device.clone();

            if let Some(permission) = permission::certain_denial(source) {
                return Err(CmdError::Permission {
                    permission,
                    message:
                        "麦克风权限已被拒绝，系统不会再弹窗。请到「权限」页打开系统设置重新允许。"
                            .into(),
                });
            }

            match session::start(app, cfg, source, device) {
                Ok(()) => {
                    if let Err(e) =
                        prefs::update(app, |p| p.apply_session(source.as_str(), device_for_prefs))
                    {
                        tracing::error!("偏好保存失败，本次设置不会被记住：{e}");
                    }
                    Ok(json!(session::TaskStatus::Capturing))
                }
                Err(msg) => match permission::certain_denial(source) {
                    Some(permission) => Err(CmdError::Permission {
                        permission,
                        message: msg,
                    }),
                    None => Err(CmdError::Failed(msg)),
                },
            }
        }
        "session_stop" => session::stop(app)
            .map(|()| json!(session::TaskStatus::Ready))
            .map_err(CmdError::Failed),

        "session_snapshot" => Ok(json!(app.state::<session::SessionState>().snapshot())),

        "session_status" => Ok(json!(app.state::<session::SessionState>().info())),

        "session_clear" => {
            session::clear(app);
            Ok(json!(null))
        }

        "hotkey_failures" => Ok(json!(app.state::<shortcut::HotkeyState>().failures())),

        "shortcuts_get" => Ok(json!(shortcut::list(app))),

        "shortcuts_set" => {
            let id = args
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| CmdError::BadArgs("缺少 id".into()))?;
            let accelerator = args
                .get("accelerator")
                .and_then(Value::as_str)
                .ok_or_else(|| CmdError::BadArgs("缺少 accelerator".into()))?;

            shortcut::rebind(app, id, accelerator)
                .map(|list| json!(list))
                .map_err(CmdError::Failed)
        }

        other => Err(CmdError::Unknown(other.to_string())),
    }
}

fn parse_source(s: &str) -> Option<audio::AudioSourceKind> {
    match s {
        "mic" => Some(audio::AudioSourceKind::Mic),
        "loopback" => Some(audio::AudioSourceKind::Loopback),
        "mix" => Some(audio::AudioSourceKind::Mix),
        _ => None,
    }
}

fn app_info() -> Value {
    json!({
        "name": "AudioScript",
        "version": env!("CARGO_PKG_VERSION"),
        "platform": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "builtAtMs": exe_mtime_ms(),
    })
}

const REPO_URL: &str = "https://github.com/niyongsheng/AudioScript";

fn export_dir(app: &AppHandle) -> Result<std::path::PathBuf, CmdError> {
    app.path()
        .desktop_dir()
        .or_else(|_| app.path().home_dir())
        .map_err(|e| CmdError::Failed(format!("找不到可写入的位置：{e}")))
}

fn safe_stamp(stamp: &str) -> String {
    let s: String = stamp
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    if s.trim_matches('-').is_empty() {
        "export".into()
    } else {
        s
    }
}

fn export_transcript(app: &AppHandle, stamp: &str) -> Result<std::path::PathBuf, CmdError> {
    let segments = app.state::<session::SessionState>().snapshot();
    let p = prefs::load(app);
    let direction = match p.translate_engine {
        TranslateEngine::Off => None,
        _ => Some(if target_lang_for(&p.source_lang) == "en" {
            "中文 → 英语"
        } else {
            "英语 → 中文"
        }),
    };

    let md = crate::transcript::to_markdown(&segments, stamp, direction);
    let file = export_dir(app)?.join(format!("AudioScript-转写-{}.md", safe_stamp(stamp)));
    std::fs::write(&file, md)
        .map_err(|e| CmdError::Failed(format!("写入 {} 失败：{e}", file.display())))?;

    tracing::info!(path = %file.display(), "转写已导出");
    Ok(file)
}

fn open_with_system(target: &std::ffi::OsStr, what: &str) -> Result<(), CmdError> {
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut cmd = std::process::Command::new("explorer");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = std::process::Command::new("xdg-open");

    cmd.arg(target)
        .spawn()
        .map(|_| ())
        .map_err(|e| CmdError::Failed(format!("无法打开{what}：{e}")))
}

fn reveal_in_file_manager(path: &std::path::Path) -> Result<(), CmdError> {
    open_with_system(path.as_os_str(), "文件管理器")
}

fn export_logs(app: &AppHandle, src: &std::path::Path) -> Result<std::path::PathBuf, CmdError> {
    let stamp = newest_log_stamp(src).unwrap_or_else(|| "unknown".into());
    let dest = export_dir(app)?.join(format!("AudioScript-日志-{stamp}"));
    let files = copy_logs(src, &dest)?;

    tracing::info!(from = %src.display(), to = %dest.display(), files, "日志已导出");
    Ok(dest)
}

fn copy_logs(src: &std::path::Path, dest: &std::path::Path) -> Result<usize, CmdError> {
    std::fs::create_dir_all(dest)
        .map_err(|e| CmdError::Failed(format!("创建 {} 失败：{e}", dest.display())))?;

    let entries =
        std::fs::read_dir(src).map_err(|e| CmdError::Failed(format!("读取日志目录失败：{e}")))?;

    let mut copied = 0usize;
    for entry in entries.flatten() {
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        let Some(name) = p.file_name() else { continue };
        std::fs::copy(&p, dest.join(name))
            .map_err(|e| CmdError::Failed(format!("复制 {} 失败：{e}", p.display())))?;
        copied += 1;
    }

    if copied == 0 {
        return Err(CmdError::Failed(format!(
            "{} 中没有日志文件，无可导出",
            src.display()
        )));
    }
    Ok(copied)
}

fn newest_log_stamp(dir: &std::path::Path) -> Option<String> {
    let mut stamps: Vec<String> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter_map(|n| n.strip_prefix("audioscript.log.").map(str::to_owned))
        .collect();
    stamps.sort();
    stamps.pop()
}

fn model_info(app: &AppHandle) -> Value {
    use crate::translate::local_nmt::{MODEL_EN_ZH, MODEL_ZH_EN};

    let asr_root = paths::resolve_models_root(app).ok();
    let mt_root = paths::resolve_mt_model_root(app).ok();

    let asr = asr_root.as_ref().map(|root| {
        let dir = root.path.join(crate::asr::DEFAULT_MODEL_DIR_NAME);
        json!({
            "name": crate::asr::DEFAULT_MODEL_DIR_NAME,
            "path": dir.display().to_string(),
            "installed": dir.is_dir(),
            "complete": paths::model_dir_is_complete(&dir).is_ok(),
            "via": root.via.as_str(),
            "license": "Apache-2.0",
            "source": "k2-fsa/sherpa-onnx",
        })
    });

    let mt = mt_root.as_ref().map(|root| {
        let one = |dir_name: &str, license: &str, attribution: bool| {
            let d = root.join(dir_name);
            json!({
                "name": dir_name,
                "path": d.display().to_string(),
                "installed": d.is_dir(),
                "license": license,
                "requiresAttribution": attribution,
            })
        };
        json!([
            one(MODEL_EN_ZH, "Apache-2.0", false),
            one(MODEL_ZH_EN, "CC-BY-4.0", true),
        ])
    });

    json!({ "asr": asr, "translate": mt })
}

fn exe_mtime_ms() -> Option<u64> {
    let exe = std::env::current_exe().ok()?;
    let mtime = std::fs::metadata(exe).ok()?.modified().ok()?;
    mtime
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("audioscript-logs-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn copy_logs_copies_every_file() {
        let src = tmp("src");
        std::fs::write(src.join("audioscript.log.2026-10-15"), b"a").unwrap();
        std::fs::write(src.join("audioscript.log.2026-10-14"), b"b").unwrap();
        let dest = src.join("out");

        assert_eq!(copy_logs(&src, &dest).unwrap(), 2);
        assert!(dest.join("audioscript.log.2026-10-15").is_file());
        assert!(dest.join("audioscript.log.2026-10-14").is_file());

        std::fs::create_dir_all(src.join("sub")).unwrap();
        let dest2 = src.join("out2");
        assert_eq!(copy_logs(&src, &dest2).unwrap(), 2, "子目录不该被算进去");

        std::fs::remove_dir_all(&src).ok();
    }

    #[test]
    fn copy_logs_fails_on_empty_source_instead_of_succeeding_silently() {
        let src = tmp("empty");
        let e = copy_logs(&src, &src.join("out")).unwrap_err();
        assert!(e.to_string().contains("没有日志文件"), "{e}");
        std::fs::remove_dir_all(&src).ok();
    }

    #[test]
    fn newest_stamp_picks_the_latest_date() {
        let src = tmp("stamp");
        for d in ["2026-10-13", "2026-10-15", "2026-10-14"] {
            std::fs::write(src.join(format!("audioscript.log.{d}")), b"x").unwrap();
        }
        std::fs::write(src.join("notes.txt"), b"x").unwrap();

        assert_eq!(newest_log_stamp(&src).as_deref(), Some("2026-10-15"));
        std::fs::remove_dir_all(&src).ok();
    }

    #[test]
    fn newest_stamp_is_none_when_there_are_no_logs() {
        let src = tmp("nostamp");
        assert_eq!(newest_log_stamp(&src), None);
        std::fs::remove_dir_all(&src).ok();
    }
}
