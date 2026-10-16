use tauri::WebviewWindow;
use tauri_nspanel::{tauri_panel, PanelHandle, WebviewWindowExt};

tauri_panel! {
    panel!(OverlayPanel {
        config: {
            can_become_key_window: false,
            can_become_main_window: false,
            is_floating_panel: true,
            hides_on_deactivate: false,
            becomes_key_only_if_needed: true
        }
    })
}

pub fn convert(window: &WebviewWindow) -> tauri::Result<PanelHandle<tauri::Wry>> {
    window.to_panel::<OverlayPanel>()
}
