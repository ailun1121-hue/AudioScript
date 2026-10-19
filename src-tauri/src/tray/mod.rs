use std::sync::Mutex;

use tauri::{
    menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

use crate::config::OverlayMode;
use crate::{lifecycle, overlay};

pub const TRAY_ID: &str = "audioscript-tray";

mod id {
    pub const SHOW_MAIN: &str = "show_main";
    pub const STOP_SESSION: &str = "stop_session";
    pub const TOGGLE_OVERLAY: &str = "toggle_overlay";
    pub const OVERLAY_LOCKED: &str = "overlay_locked";
    pub const QUIT: &str = "quit";
}

pub struct TrayState {
    #[allow(dead_code)]
    icon: Mutex<tauri::tray::TrayIcon<tauri::Wry>>,
    locked_item: tauri::menu::CheckMenuItem<tauri::Wry>,
}

pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let show_main = MenuItemBuilder::with_id(id::SHOW_MAIN, "显示主窗口").build(app)?;
    let stop_session = MenuItemBuilder::with_id(id::STOP_SESSION, "停止转写").build(app)?;
    let toggle_overlay =
        MenuItemBuilder::with_id(id::TOGGLE_OVERLAY, "显示 / 隐藏悬浮窗").build(app)?;

    let locked_item = CheckMenuItemBuilder::with_id(id::OVERLAY_LOCKED, "点击穿透（锁定）")
        .checked(app.state::<overlay::OverlayState>().mode_is_click_through())
        .build(app)?;

    let quit = MenuItemBuilder::with_id(id::QUIT, "退出 AudioScript").build(app)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;

    let menu = MenuBuilder::new(app)
        .items(&[
            &show_main,
            &stop_session,
            &sep1,
            &toggle_overlay,
            &locked_item,
            &sep2,
            &quit,
        ])
        .build()?;

    let icon = TrayIconBuilder::with_id(TRAY_ID)
        .icon(tray_icon(app)?)
        .icon_as_template(true)
        .tooltip("AudioScript 声稿")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(on_tray_event)
        .build(app)?;

    app.manage(TrayState {
        icon: Mutex::new(icon),
        locked_item,
    });

    Ok(())
}

pub fn sync_mode(app: &AppHandle, mode: OverlayMode) {
    let Some(state) = app.try_state::<TrayState>() else {
        return;
    };
    if let Err(e) = state.locked_item.set_checked(mode.is_click_through()) {
        tracing::error!("同步托盘勾选态失败：{e}");
    }
}

fn tray_icon(app: &AppHandle) -> tauri::Result<tauri::image::Image<'static>> {
    const TEMPLATE: &[u8] = include_bytes!("../../icons/tray-template.png");

    if let Ok(img) = tauri::image::Image::from_bytes(TEMPLATE) {
        return Ok(img);
    }

    tracing::warn!("托盘模板图加载失败，退回应用图标（在菜单栏里可能显示异常）");
    let icon = app
        .default_window_icon()
        .ok_or_else(|| tauri::Error::AssetNotFound("没有可用的托盘图标".into()))?;

    Ok(tauri::image::Image::new_owned(
        icon.rgba().to_vec(),
        icon.width(),
        icon.height(),
    ))
}

fn on_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    match event.id().as_ref() {
        id::SHOW_MAIN => lifecycle::show_main(app),
        id::STOP_SESSION => match crate::session::stop(app) {
            Ok(()) => tracing::info!("托盘：已请求停止转写"),
            Err(e) => tracing::error!("托盘停止转写失败：{e}"),
        },
        id::TOGGLE_OVERLAY => {
            if let Err(e) = overlay::toggle(app) {
                tracing::error!("{e}");
            }
        }
        id::OVERLAY_LOCKED => {
            let next = if app
                .try_state::<TrayState>()
                .map(|s| s.locked_item.is_checked().unwrap_or(false))
                .unwrap_or(false)
            {
                OverlayMode::Locked
            } else {
                OverlayMode::Edit
            };
            if let Err(e) = overlay::set_mode(app, next) {
                tracing::error!("{e}");
                sync_mode(app, app.state::<overlay::OverlayState>().mode());
            }
        }
        id::QUIT => lifecycle::quit_app(app),
        other => tracing::debug!("未处理的托盘菜单项：{other}"),
    }
}

fn on_tray_event(tray: &tauri::tray::TrayIcon, event: TrayIconEvent) {
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        lifecycle::show_main(tray.app_handle());
    }
}
