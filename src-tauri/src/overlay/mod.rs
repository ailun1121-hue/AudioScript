pub mod geometry;
pub mod native;
#[cfg(target_os = "macos")]
pub mod panel;

#[cfg(target_os = "macos")]
pub mod watchdog;

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::config::{OverlayConfig, OverlayMode};

pub const LABEL: &str = "overlay";

pub const ENV_OVERLAY_LEVEL: &str = "AUDIOSCRIPT_OVERLAY_LEVEL";

pub const EVENT_OVERLAY_STATE: &str = "overlay://state";

pub const EVENT_OVERLAY_VISIBILITY: &str = "overlay://visibility";

#[derive(Debug, Default)]
pub struct OverlayState {
    config: Mutex<OverlayConfig>,
    applied_click_through: Mutex<Option<bool>>,
}

impl OverlayState {
    pub fn config(&self) -> OverlayConfig {
        self.config
            .lock()
            .expect("overlay config mutex poisoned")
            .clone()
    }

    pub fn mode(&self) -> OverlayMode {
        self.config().mode
    }

    pub fn mode_is_click_through(&self) -> bool {
        self.mode().is_click_through()
    }

    fn replace(&self, cfg: OverlayConfig) {
        *self.config.lock().expect("overlay config mutex poisoned") = cfg;
    }

    fn set_mode(&self, mode: OverlayMode) {
        self.config
            .lock()
            .expect("overlay config mutex poisoned")
            .mode = mode;
    }

    fn set_enable(&self, enable: bool) {
        self.config
            .lock()
            .expect("overlay config mutex poisoned")
            .enable = enable;
    }

    fn needs_click_through_write(&self, want: bool) -> bool {
        let mut cur = self
            .applied_click_through
            .lock()
            .expect("overlay click-through mutex poisoned");
        if *cur == Some(want) {
            return false;
        }
        *cur = Some(want);
        true
    }
}

pub(crate) enum Frame {
    #[cfg(target_os = "macos")]
    Panel(tauri_nspanel::PanelHandle<tauri::Wry>),
    Window(Box<WebviewWindow>),
}

impl Frame {
    pub(crate) fn get(app: &AppHandle) -> Result<Self, String> {
        #[cfg(target_os = "macos")]
        {
            use tauri_nspanel::ManagerExt;
            if let Ok(p) = app.get_webview_panel(LABEL) {
                return Ok(Self::Panel(p));
            }
        }

        app.get_webview_window(LABEL)
            .map(|w| Self::Window(Box::new(w)))
            .ok_or_else(|| format!("悬浮窗（label={LABEL}）不存在"))
    }

    fn show(&self) -> Result<(), String> {
        match self {
            #[cfg(target_os = "macos")]
            Self::Panel(p) => {
                p.show();
                Ok(())
            }
            Self::Window(w) => w.show().map_err(|e| format!("显示悬浮窗失败：{e}")),
        }
    }

    fn hide(&self) -> Result<(), String> {
        match self {
            #[cfg(target_os = "macos")]
            Self::Panel(p) => {
                p.hide();
                Ok(())
            }
            Self::Window(w) => w.hide().map_err(|e| format!("隐藏悬浮窗失败：{e}")),
        }
    }

    fn is_visible(&self) -> bool {
        match self {
            #[cfg(target_os = "macos")]
            Self::Panel(p) => p.is_visible(),
            Self::Window(w) => w.is_visible().unwrap_or(false),
        }
    }

    pub(crate) fn window(&self) -> Option<WebviewWindow> {
        match self {
            #[cfg(target_os = "macos")]
            Self::Panel(p) => p.to_window(),
            Self::Window(w) => Some((**w).clone()),
        }
    }

    pub(crate) fn geometry(&self) -> Option<crate::config::WindowGeometry> {
        use crate::config::{WindowGeometry, WindowPos, WindowSize};
        let w = self.window()?;
        let pos = w.outer_position().ok()?;
        let size = w.outer_size().ok()?;
        Some(WindowGeometry {
            position: WindowPos { x: pos.x, y: pos.y },
            size: WindowSize {
                width: size.width,
                height: size.height,
            },
        })
    }

    pub(crate) fn set_geometry(&self, g: &crate::config::WindowGeometry) -> Result<(), String> {
        let w = self.window().ok_or("悬浮窗底层窗口不可用")?;
        w.set_size(tauri::PhysicalSize::new(g.size.width, g.size.height))
            .map_err(|e| format!("设置悬浮窗尺寸失败：{e}"))?;
        w.set_position(tauri::PhysicalPosition::new(g.position.x, g.position.y))
            .map_err(|e| format!("设置悬浮窗位置失败：{e}"))?;
        Ok(())
    }

    fn set_click_through(&self, on: bool) -> Result<(), String> {
        match self {
            #[cfg(target_os = "macos")]
            Self::Panel(p) => {
                p.set_ignores_mouse_events(on);
                Ok(())
            }
            Self::Window(w) => w
                .set_ignore_cursor_events(on)
                .map_err(|e| format!("设置鼠标穿透失败：{e}")),
        }
    }

    fn label(&self) -> &str {
        match self {
            #[cfg(target_os = "macos")]
            Self::Panel(p) => p.label(),
            Self::Window(w) => w.label(),
        }
    }
}

pub fn toggle(app: &AppHandle) -> Result<bool, String> {
    let frame = Frame::get(app)?;
    let visible = frame.is_visible();

    if visible {
        frame.hide()?;
    } else {
        frame.show()?;
        apply_fullscreen_overlay(app, &frame);
    }

    let now = !visible;
    set_enable(app, now);
    broadcast_visibility(app, now);

    Ok(now)
}

/// 可见性就是 `enable`：托盘 / 热键 / 主窗口按钮三条路径都汇到这里，用户藏起来之后重启不会自己冒出来。
fn set_enable(app: &AppHandle, enable: bool) {
    let state = app.state::<OverlayState>();
    state.set_enable(enable);
    if let Err(e) = crate::prefs::update(app, |p| p.overlay.enable = enable) {
        tracing::error!("悬浮窗可见性保存失败：{e}（重启后会回到上次落盘的状态）");
    }
    broadcast(app, &state);
}

pub fn is_visible(app: &AppHandle) -> bool {
    Frame::get(app).map(|f| f.is_visible()).unwrap_or(false)
}

fn broadcast_visibility(app: &AppHandle, visible: bool) {
    if let Err(e) = app.emit(EVENT_OVERLAY_VISIBILITY, visible) {
        tracing::debug!("广播悬浮窗可见性失败（可能没有窗口在监听）：{e}");
    }
}

pub fn set_mode(app: &AppHandle, mode: OverlayMode) -> Result<OverlayMode, String> {
    let frame = Frame::get(app)?;
    let state = app.state::<OverlayState>();

    let want = mode.is_click_through();
    if state.needs_click_through_write(want) {
        if let Err(e) = frame.set_click_through(want) {
            *state
                .applied_click_through
                .lock()
                .expect("overlay click-through mutex poisoned") = None;
            return Err(format!("{e}（mode={}）", mode.as_str()));
        }
    }

    state.set_mode(mode);
    if let Err(e) = crate::prefs::update(app, |p| p.overlay.mode = mode) {
        tracing::error!("悬浮窗交互模式保存失败：{e}（重启后会回到上次落盘的模式）");
    }
    broadcast(app, &state);
    Ok(mode)
}

pub fn config(app: &AppHandle) -> OverlayConfig {
    app.state::<OverlayState>().config()
}

pub fn set_config(app: &AppHandle, cfg: OverlayConfig) -> Result<OverlayConfig, String> {
    let cfg = cfg.sanitized();
    let state = app.state::<OverlayState>();

    if cfg.mode != state.mode() {
        set_mode(app, cfg.mode)?;
    }

    #[cfg(target_os = "macos")]
    if !cfg.always_on_top {
        tracing::warn!(
            "always_on_top=false 在 macOS 上不被采纳：跨全屏置顶依赖强制的 NSScreenSaverWindowLevel"
        );
    }

    state.replace(cfg.clone());
    broadcast(app, &state);
    Ok(cfg)
}

fn broadcast(app: &AppHandle, state: &OverlayState) {
    if let Err(e) = app.emit(EVENT_OVERLAY_STATE, state.config()) {
        tracing::debug!("广播悬浮窗状态失败（可能没有窗口在监听）：{e}");
    }
}

pub fn init(
    app: &AppHandle,
    cfg: OverlayConfig,
    geometry: Option<crate::config::WindowGeometry>,
) -> Result<(), String> {
    let frame = Frame::get(app)?;

    geometry::restore(app, geometry);

    let state = app.state::<OverlayState>();
    state.replace(cfg.sanitized());
    let mode = state.mode();

    frame.set_click_through(mode.is_click_through())?;
    state.needs_click_through_write(mode.is_click_through());

    apply_fullscreen_overlay(app, &frame);

    broadcast(app, &state);

    let applied = state.config();
    tracing::info!(
        font_size = applied.font_size,
        opacity = applied.opacity,
        max_lines = applied.max_lines,
        font_color = %applied.font_color,
        bg_color = %applied.bg_color,
        mode = applied.mode.as_str(),
        "悬浮窗外观已应用"
    );

    // 显示 MUST 排在 `state.replace()` 之后：窗口 spec 里 `visible: false`，出场前外观与几何必须已就位。
    if applied.enable {
        if let Err(e) = frame.show() {
            tracing::error!("悬浮窗启动显示失败：{e}（可在悬浮窗页手动显示）");
        }
    }

    #[cfg(target_os = "macos")]
    watchdog::start(app.clone());

    geometry::start_watch(app);

    Ok(())
}

pub fn on_page_loaded(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        let Some(window) = app.get_webview_window(LABEL) else {
            tracing::error!("page-load 回调里找不到悬浮窗，无法换类");
            return;
        };

        let handle = app.clone();
        let window_for_convert = window.clone();
        if let Err(e) =
            window.run_on_main_thread(move || match panel::convert(&window_for_convert) {
                Ok(_) => {
                    tracing::info!("悬浮窗已换类为 NSPanel（跨全屏的前提）");
                    if let Ok(frame) = Frame::get(&handle) {
                        apply_fullscreen_overlay(&handle, &frame);
                    }
                }
                Err(e) => tracing::error!(
                    "悬浮窗换类失败：{e}\n\
                 → 后果：其他应用全屏时悬浮窗不可见（看视频读字幕的主场景会失效）"
                ),
            })
        {
            tracing::error!("无法投递换类任务到主线程：{e}");
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
    }
}

fn apply_fullscreen_overlay(app: &AppHandle, frame: &Frame) {
    #[cfg(target_os = "macos")]
    {
        let Frame::Panel(p) = frame else {
            tracing::debug!("悬浮窗尚未换类，跳过全屏属性设置");
            return;
        };

        let panel = p.clone();
        let handle = app.clone();
        if let Err(e) = app.run_on_main_thread(move || {
            native::macos::apply_overlay_panel_attributes(&panel);
            match debug_state(&handle) {
                Ok(s) => tracing::info!(state = %s, "悬浮窗原生属性已应用"),
                Err(e) => tracing::warn!("属性已设置但状态回读失败：{e}"),
            }
        }) {
            tracing::error!("无法投递原生属性设置到主线程：{e}");
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, frame);
    }
}

pub fn debug_state(app: &AppHandle) -> Result<serde_json::Value, String> {
    let frame = Frame::get(app)?;

    let mut out = serde_json::json!({
        "label": frame.label(),
        "visible": frame.is_visible(),
        "clickThrough": app.state::<OverlayState>().mode_is_click_through(),
    });

    if let Some(g) = frame.geometry() {
        out["geometry"] = serde_json::json!({
            "x": g.position.x,
            "y": g.position.y,
            "width": g.size.width,
            "height": g.size.height,
        });
    }

    #[cfg(target_os = "macos")]
    {
        out["convertedToPanel"] = serde_json::json!(matches!(frame, Frame::Panel(_)));

        if let Frame::Panel(p) = &frame {
            out["level"] = serde_json::json!(native::macos::current_level(p));
            out["reallyPainted"] = serde_json::json!(native::macos::really_painted(p));
            out["onActiveSpace"] = serde_json::json!(native::macos::on_active_space(p));
            out["styleMask"] = serde_json::json!(format!("{:?}", p.as_panel().styleMask()));
        }
    }

    Ok(out)
}
