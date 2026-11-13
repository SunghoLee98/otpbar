//! 사용자 설정. 비밀 정보는 담지 않는다.

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::util;

/// 전역 단축키의 기본 조합. macOS는 `⌥⇧⌘O`, Windows·Linux는 `Ctrl+Alt+Shift+O`.
///
/// 전역 단축키는 등록하는 순간 다른 모든 앱에서 그 조합을 빼앗는다. 그래서 수정키
/// 셋을 모두 쓴다. 둘만 쓰는 조합은 이미 주인이 있는 경우가 많다.
/// `O`(OTP)는 기억하기 쉬워서 남겼다.
pub const DEFAULT_HOTKEY: &str = "CmdOrCtrl+Alt+Shift+O";

/// 2.3.0의 기본 조합. 브라우저 북마크 관리자(Chrome·Edge·Firefox), VS Code의
/// 기호로 이동, IntelliJ 계열의 파일 열기, macOS Finder의 문서 폴더 열기와 겹쳤다.
const LEGACY_HOTKEY: &str = "CmdOrCtrl+Shift+O";

/// `#[serde(default)]`가 구조체 전체에 붙어 있어야 한다. 이것이 없으면 필드를 하나
/// 더할 때마다 옛 설정 파일의 파싱이 실패하고, [`Settings::load`]가
/// `unwrap_or_default()`로 **사용자 설정 전체를 초기화**한다.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// 마지막 조작 후 이 시간(초)이 지나면 자동으로 잠근다. 0이면 잠그지 않는다.
    /// 기본은 0이다. 자동 해제를 쓰는 기본 구성에서는 잠가도 곧바로 다시 열리므로
    /// 의미가 없고, 마스터 암호 보호를 켠 사용자만 이 값을 올린다.
    pub auto_lock_secs: u64,
    /// 코드를 가린 채 표시하고, 누를 때만 보여 준다.
    pub mask_codes: bool,
    /// 알림 본문에 코드를 넣는다(기본 꺼짐 — 잠금 화면 미리보기 노출 방지).
    pub show_code_in_notification: bool,
    /// 복사 후 이 시간(초)이 지나면 클립보드를 비운다. 0이면 비우지 않는다.
    pub clipboard_clear_secs: u64,
    /// 업데이트를 자동으로 확인한다.
    pub auto_update_check: bool,
    /// 전역 단축키로 마지막에 쓴 계정의 코드를 복사한다.
    pub hotkey_enabled: bool,
    /// 전역 단축키 조합. Tauri 표기법을 쓴다(예: `CmdOrCtrl+Shift+O`).
    pub hotkey: String,
    /// 단축키가 꺼낼 계정. 코드를 복사할 때마다 갱신한다.
    pub last_used_id: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            auto_lock_secs: 0,
            mask_codes: true,
            show_code_in_notification: false,
            clipboard_clear_secs: 20,
            auto_update_check: true,
            hotkey_enabled: true,
            hotkey: DEFAULT_HOTKEY.into(),
            last_used_id: None,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let path = util::settings_path();
        let mut s: Settings = std::fs::read(&path)
            .ok()
            .and_then(|raw| serde_json::from_slice(&raw).ok())
            .unwrap_or_default();
        if s.migrate() {
            // 저장에 실패해도 메모리의 값은 이미 새 조합이다. 다음 실행에서 다시 옮긴다.
            s.save().ok();
        }
        s
    }

    /// 옛 버전이 남긴 값을 고친다. 바꾼 것이 있으면 `true`.
    ///
    /// 2.3.0은 조합을 바꾸는 화면이 없었으므로, 옛 기본값이 저장돼 있다면 사용자가
    /// 고른 것이 아니라 기본값이 그대로 저장된 것이다. 새 기본값으로 옮겨도 된다.
    fn migrate(&mut self) -> bool {
        if self.hotkey == LEGACY_HOTKEY {
            self.hotkey = DEFAULT_HOTKEY.into();
            return true;
        }
        false
    }

    pub fn save(&self) -> Result<()> {
        util::write_private(&util::settings_path(), &serde_json::to_vec_pretty(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 필드를 더해도 옛 설정 파일의 값이 살아남아야 한다.
    /// `#[serde(default)]`가 빠지면 `load()`가 통째로 기본값으로 되돌린다.
    #[test]
    fn old_settings_survive_new_fields() {
        let raw = r#"{
            "auto_lock_secs": 300,
            "mask_codes": false,
            "show_code_in_notification": true,
            "clipboard_clear_secs": 45,
            "auto_update_check": false
        }"#;
        let s: Settings = serde_json::from_str(raw).expect("옛 형식도 읽혀야 한다");
        assert_eq!(s.auto_lock_secs, 300);
        assert!(!s.mask_codes, "사용자가 끈 값이 유지되어야 한다");
        assert!(s.show_code_in_notification);
        assert_eq!(s.clipboard_clear_secs, 45);
        assert!(!s.auto_update_check);
        // 새 필드는 기본값으로 채워진다
        assert!(s.hotkey_enabled);
        assert_eq!(s.hotkey, DEFAULT_HOTKEY);
        assert_eq!(s.last_used_id, None);
    }

    /// 2.3.0이 저장한 옛 기본 조합은 새 기본 조합으로 옮긴다.
    #[test]
    fn legacy_hotkey_is_migrated() {
        let raw = r#"{ "hotkey_enabled": false, "hotkey": "CmdOrCtrl+Shift+O" }"#;
        let mut s: Settings = serde_json::from_str(raw).unwrap();
        assert!(s.migrate());
        assert_eq!(s.hotkey, DEFAULT_HOTKEY);
        assert!(!s.hotkey_enabled, "켜고 끈 상태는 그대로 둔다");
        assert!(!s.migrate(), "두 번째에는 바꿀 것이 없다");
    }

    /// 사용자가 직접 정한 조합은 건드리지 않는다.
    #[test]
    fn custom_hotkey_is_kept() {
        let mut s = Settings {
            hotkey: "Ctrl+Alt+K".into(),
            ..Settings::default()
        };
        assert!(!s.migrate());
        assert_eq!(s.hotkey, "Ctrl+Alt+K");
    }

    #[test]
    fn defaults_are_conservative() {
        let s = Settings::default();
        assert!(s.mask_codes, "코드는 기본으로 가린다");
        assert_eq!(
            s.auto_lock_secs, 0,
            "기본 구성에서는 자동 잠금을 걸지 않는다"
        );
        assert!(!s.show_code_in_notification, "알림에 코드를 넣지 않는다");
        assert!(s.clipboard_clear_secs > 0, "클립보드를 비운다");
    }
}
