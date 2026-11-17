//! 전역 단축키. 어느 앱에 있든 빠른 복사 창([`crate::quick`])을 띄운다.
//!
//! 이 앱을 쓰는 이유는 "코드를 빨리 꺼내는 것"인데, 종전에는 메뉴를 열고 계정을 찾아
//! 눌러야 했다. 이제 단축키를 누르고 숫자 하나를 누르면 끝난다.

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::state::AppState;
use crate::ui;

/// 설정에 적힌 단축키를 등록한다. 이미 다른 앱이 쓰고 있으면 조용히 실패한다.
pub fn register(app: &AppHandle) {
    let settings = app.state::<AppState>().settings();
    if !settings.hotkey_enabled {
        return;
    }
    let combo = settings.hotkey.clone();
    let result = app
        .global_shortcut()
        .on_shortcut(combo.as_str(), |app, _shortcut, event| {
            // 누를 때 한 번만 반응한다. 떼는 순간까지 처리하면 두 번 복사된다.
            if event.state == ShortcutState::Pressed {
                let app = app.clone();
                std::thread::spawn(move || crate::quick::toggle(&app));
            }
        });
    match result {
        Ok(()) => crate::log!("전역 단축키를 등록했습니다: {combo}"),
        // 다른 앱이 선점한 조합일 수 있다. 앱 전체를 막을 일은 아니다.
        Err(e) => crate::log!("전역 단축키를 등록하지 못했습니다({combo}): {e}"),
    }
}

/// 등록을 해제한다. 설정을 끄거나 조합을 바꿀 때 부른다.
pub fn unregister(app: &AppHandle) {
    let combo = app.state::<AppState>().settings().hotkey;
    if let Err(e) = app.global_shortcut().unregister(combo.as_str()) {
        crate::log!("전역 단축키를 해제하지 못했습니다: {e}");
    }
}

/// 설정을 뒤집고 등록 상태를 맞춘다.
pub fn toggle(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut s = state.settings();
    s.hotkey_enabled = !s.hotkey_enabled;
    let now = s.hotkey_enabled;
    let combo = s.hotkey.clone();
    state.set_settings(s).ok();

    if now {
        register(app);
        ui::notify(app, "OTPBar", &format!("{combo} 로 빠른 복사 창을 엽니다."));
    } else {
        unregister(app);
        ui::notify(app, "OTPBar", "전역 단축키를 껐습니다.");
    }
    ui::refresh(app);
}
