use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::AppHandle;

const CHECK_INTERVAL: Duration = Duration::from_secs(2);

static RUNNING: AtomicBool = AtomicBool::new(false);

pub fn start(app: AppHandle) {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    std::thread::Builder::new()
        .name("overlay-watchdog".into())
        .spawn(move || {
            tracing::debug!("悬浮窗自检循环已启动（间隔 {:?}）", CHECK_INTERVAL);
            while RUNNING.load(Ordering::SeqCst) {
                std::thread::sleep(CHECK_INTERVAL);

                let handle = app.clone();
                if let Err(e) = app.run_on_main_thread(move || check_and_repair(&handle)) {
                    tracing::warn!("自检无法投递到主线程：{e}");
                }
            }
        })
        .ok();
}

fn check_and_repair(app: &AppHandle) {
    use crate::overlay::{Frame, LABEL};

    let Ok(frame) = Frame::get(app) else {
        return;
    };

    let Frame::Panel(panel) = &frame else {
        return;
    };

    if !panel.is_visible() {
        return;
    }

    let on_space = crate::overlay::native::macos::on_active_space(panel);
    if !on_space {
        tracing::info!("悬浮窗掉出了当前 Space，重新断言原生属性");
        crate::overlay::native::macos::apply_overlay_panel_attributes(panel);
    }

    repair_frame_if_offscreen(panel, LABEL);
}

fn repair_frame_if_offscreen(panel: &tauri_nspanel::PanelHandle<tauri::Wry>, label: &str) {
    let ns_panel = panel.as_panel();
    let frame = ns_panel.frame();

    let screens = tauri_nspanel::objc2_app_kit::NSScreen::screens(
        tauri_nspanel::objc2_foundation::MainThreadMarker::new()
            .expect("check_and_repair 必须在主线程调用"),
    );

    let mut intersects_any = false;
    for screen in screens.iter() {
        let sf = screen.frame();
        let x_overlap = frame.origin.x < sf.origin.x + sf.size.width
            && frame.origin.x + frame.size.width > sf.origin.x;
        let y_overlap = frame.origin.y < sf.origin.y + sf.size.height
            && frame.origin.y + frame.size.height > sf.origin.y;
        if x_overlap && y_overlap {
            intersects_any = true;
            break;
        }
    }

    if intersects_any {
        return;
    }

    let Some(main) = screens.firstObject() else {
        return;
    };
    let visible = main.visibleFrame();

    let x = visible.origin.x + (visible.size.width - frame.size.width) / 2.0;
    let y = visible.origin.y + visible.size.height - frame.size.height - 80.0;

    ns_panel.setFrameOrigin(tauri_nspanel::objc2_foundation::NSPoint { x, y });
    tracing::warn!(
        label,
        "悬浮窗原先完全在屏幕之外（多半是显示器拓扑变了），已搬回主屏"
    );
}
