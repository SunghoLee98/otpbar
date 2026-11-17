//! 빠른 복사 창. 단축키를 누르면 화면 가운데에 반투명한 작은 창이 뜨고,
//! 숫자 1~9를 누르면 그 번호의 코드를 클립보드에 넣고 사라진다.
//!
//! 검색도, 단추도 두지 않는다. 할 수 있는 일은 세 가지뿐이다.
//!
//! - 숫자(1~9) 또는 클릭: 복사하고 닫는다
//! - Esc: 닫는다
//! - 창 밖을 누르거나 다른 앱으로 가면: 닫는다
//!
//! 창은 한 번 만든 뒤 숨겨 두고 다시 쓴다. 매번 새로 만들면 웹뷰가 뜨는 동안
//! 단축키를 누른 손이 기다려야 한다.

use otpbar_core::ipc::ListItem;
use tauri::window::{Effect, EffectState, EffectsBuilder};
use tauri::{AppHandle, LogicalSize, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::state::AppState;

pub const WINDOW_LABEL: &str = "quick";

/// 번호를 붙여 보여 줄 수 있는 최대 계정 수(1~9).
const MAX_ITEMS: usize = 9;
const WIDTH: f64 = 400.0;
const ROW_HEIGHT: f64 = 44.0;
/// 위아래 여백과 안내 한 줄.
const CHROME_HEIGHT: f64 = 48.0;

fn height_for(rows: usize) -> f64 {
    CHROME_HEIGHT + ROW_HEIGHT * rows.clamp(1, MAX_ITEMS) as f64
}

/// 단축키를 눌렀을 때. 떠 있으면 닫고, 아니면 연다.
pub fn toggle(app: &AppHandle) {
    let state = app.state::<AppState>();
    if !state.is_unlocked() {
        crate::ui::notify(
            app,
            "OTPBar",
            "금고가 잠겨 있습니다. 먼저 잠금을 해제하십시오.",
        );
        crate::tray::unlock_flow(app);
        return;
    }
    let rows = state.account_count();
    if rows == 0 {
        crate::ui::notify(app, "OTPBar", "등록된 계정이 없습니다.");
        return;
    }

    let handle = app.clone();
    let result = app.run_on_main_thread(move || {
        if let Some(win) = handle.get_webview_window(WINDOW_LABEL) {
            if win.is_visible().unwrap_or(false) {
                hide_now(&handle);
                return;
            }
            win.set_size(LogicalSize::new(WIDTH, height_for(rows))).ok();
            win.center().ok();
            win.show().ok();
            win.set_focus().ok();
            return;
        }
        build(&handle, rows);
    });
    if let Err(e) = result {
        crate::log!("빠른 복사 창 요청을 주 스레드로 넘기지 못했습니다: {e}");
    }
}

fn build(app: &AppHandle, rows: usize) {
    // macOS는 HUD 재질, Windows 11은 아크릴로 뒤를 흐린다. 지원하지 않는 환경에서는
    // 무시되고, CSS의 반투명 배경만 남는다.
    let effects = EffectsBuilder::new()
        .effect(Effect::HudWindow)
        .effect(Effect::Acrylic)
        .state(EffectState::Active)
        .radius(14.0)
        .build();

    let built = WebviewWindowBuilder::new(app, WINDOW_LABEL, WebviewUrl::App("quick.html".into()))
        .title("OTPBar")
        .inner_size(WIDTH, height_for(rows))
        .center()
        .decorations(false)
        .transparent(true)
        .effects(effects)
        .shadow(true)
        .resizable(false)
        .minimizable(false)
        .maximizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible_on_all_workspaces(true)
        .focused(true)
        .build();

    match built {
        Ok(win) => {
            // 다른 곳을 누르면 닫는다. 떠 있는 채로 남으면 다음 단축키가 "닫기"가 된다.
            let h = app.clone();
            win.on_window_event(move |event| {
                if let tauri::WindowEvent::Focused(false) = event {
                    if let Some(w) = h.get_webview_window(WINDOW_LABEL) {
                        w.hide().ok();
                    }
                }
            });
            win.set_focus().ok();
            crate::log!("빠른 복사 창을 열었습니다.");
        }
        Err(e) => crate::log!("빠른 복사 창을 열지 못했습니다: {e}"),
    }
}

/// 창을 숨기고 원래 쓰던 앱으로 초점을 돌려준다.
fn hide_now(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(WINDOW_LABEL) {
        win.hide().ok();
    }
    // macOS에서는 창만 숨기면 이 앱이 계속 활성 상태로 남아, 붙여넣기가 엉뚱한 곳으로
    // 간다. 앱 자체를 숨겨야 직전 앱이 다시 앞으로 나온다. 대시보드가 떠 있으면 그 창까지
    // 숨겨지므로 그때는 건드리지 않는다.
    #[cfg(target_os = "macos")]
    {
        let dashboard_open = app
            .get_webview_window(crate::dashboard::WINDOW_LABEL)
            .and_then(|w| w.is_visible().ok())
            .unwrap_or(false);
        if !dashboard_open {
            app.hide().ok();
        }
    }
}

/// 번호를 붙일 계정과 현재 코드. 이 창은 단축키로 불렀을 때만 잠깐 뜨므로
/// "코드 가려서 표시" 설정과 관계없이 코드를 보여 준다.
#[tauri::command]
pub fn quick_list(app: AppHandle) -> Result<QuickView, String> {
    let state = app.state::<AppState>();
    if !state.is_unlocked() {
        return Ok(QuickView { items: Vec::new() });
    }
    let mut items = state.list_items(true).map_err(|e| e.to_string())?;
    items.truncate(MAX_ITEMS);
    Ok(QuickView { items })
}

/// 코드를 복사한다. 창은 화면 쪽에서 짧게 표시한 뒤 [`quick_hide`]로 닫는다.
#[tauri::command]
pub fn quick_copy(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    if !state.is_unlocked() {
        return Err("금고가 잠겨 있습니다".into());
    }
    state.touch();
    let (account, code, _) = state.code_for(&id).map_err(|e| e.to_string())?;
    state.remember_last_used(&account.id);
    crate::ui::copy_code(&app, &code, state.settings().clipboard_clear_secs);
    Ok(())
}

#[tauri::command]
pub fn quick_hide(app: AppHandle) {
    let handle = app.clone();
    app.run_on_main_thread(move || hide_now(&handle)).ok();
}

#[derive(serde::Serialize)]
pub struct QuickView {
    pub items: Vec<ListItem>,
}
