//! QR 이미지에서 계정을 가져온다. 계정마다 이름을 물어 정한 뒤 등록한다.
//!
//! 2.3.0까지는 QR에 적힌 이름을 그대로 썼고, 같은 발급자에 같은 이름이 이미 있으면 그
//! 계정을 건너뛰었다. 결과는 알림 한 줄로만 지나가서, 사용자는 코드가 등록되지 않았다는
//! 사실조차 놓치기 쉬웠다. 구글 OTP 내보내기에는 이름이 같은 계정이 흔하다(같은 이메일로
//! 만든 여러 조직의 계정 등).
//!
//! 이제 계정마다 이름 입력 상자를 띄운다. QR의 이름을 기본값으로 채워 두므로 그대로 두면
//! 예전과 같다. 이미 있는 이름이면 그 자리에서 알리고 다른 이름을 다시 묻는다.

use std::sync::atomic::{AtomicBool, Ordering};

use otpbar_core::error::Error;
use otpbar_core::vault::label;
use otpbar_core::NewAccount;
use tauri::{AppHandle, Manager};

use crate::prompt::{self, Input};
use crate::state::AppState;
use crate::ui;

/// 가져오기가 진행 중인지. 입력 상자가 여러 번 이어지므로, 그 사이 메뉴를 다시 눌러
/// 두 흐름의 질문이 뒤섞이는 일을 막는다.
static RUNNING: AtomicBool = AtomicBool::new(false);

/// QR 이미지를 골라 가져온다. 파일 선택은 운영체제 표준 패널을 쓴다.
pub(crate) fn via_dialog(app: &AppHandle) {
    let state = app.state::<AppState>();
    if !state.is_unlocked() {
        ui::notify(app, "OTPBar", "먼저 잠금을 해제하십시오.");
        return;
    }
    if RUNNING.swap(true, Ordering::SeqCst) {
        ui::notify(
            app,
            "OTPBar",
            "가져오기가 이미 진행 중입니다. 열려 있는 입력 상자를 확인하십시오.",
        );
        return;
    }
    run(app, &state);
    RUNNING.store(false, Ordering::SeqCst);
}

fn run(app: &AppHandle, state: &AppState) {
    use tauri_plugin_dialog::DialogExt;

    let picked = app
        .dialog()
        .file()
        .set_title("구글 OTP 내보내기 QR 또는 otpauth QR 이미지")
        .add_filter("이미지", &["png", "jpg", "jpeg", "bmp", "webp", "tiff"])
        .blocking_pick_files();
    let Some(paths) = picked else { return };

    let mut lines: Vec<String> = Vec::new();
    let mut pending: Vec<NewAccount> = Vec::new();
    for p in paths {
        match otpbar_core::ipc::import_source_to_accounts(&p.to_string()) {
            Ok(accounts) => pending.extend(accounts),
            Err(e) => lines.push(format!("실패: {e}")),
        }
    }

    let total = pending.len();
    let mut added = 0usize;
    for (i, account) in pending.into_iter().enumerate() {
        let shown = label(&account.issuer, &account.name);
        let ask =
            |question: &str, default: &str| prompt::text(question, default, "등록", "건너뛰기");
        match register(state, account, i + 1, total, ask) {
            Outcome::Added(name) => {
                added += 1;
                lines.push(format!("등록: {name}"));
                // 다음 질문을 기다리는 동안에도 메뉴에서 바로 쓸 수 있게 한다
                ui::refresh(app);
            }
            Outcome::Skipped => lines.push(format!("건너뜀: {shown}")),
            Outcome::Failed(e) => lines.push(format!("실패: {shown} — {e}")),
        }
    }

    ui::refresh(app);
    let summary = if lines.is_empty() {
        "가져올 계정이 없습니다.".to_string()
    } else {
        let head: Vec<&str> = lines.iter().take(4).map(|s| s.as_str()).collect();
        let more = if lines.len() > 4 {
            format!("\n… 외 {}건", lines.len() - 4)
        } else {
            String::new()
        };
        format!("{}{}", head.join("\n"), more)
    };
    ui::notify(app, &format!("가져오기 {added}건"), &summary);
}

/// 계정 하나를 등록한 결과.
enum Outcome {
    /// 등록했다. 값은 최종 표시 이름.
    Added(String),
    /// 사용자가 건너뛰었다.
    Skipped,
    Failed(Error),
}

/// 이름을 물어 계정 하나를 등록한다. 쓸 수 있는 이름을 받을 때까지 다시 묻는다.
///
/// `ask`는 문구와 기본값을 받아 입력 상자를 띄운다(시험에서는 가짜로 바꾼다).
/// 이름 검사는 비밀키 없이 하므로 계정 데이터를 복사하지 않는다. 등록은 이름이 정해진 뒤
/// 한 번만 한다.
fn register(
    state: &AppState,
    mut account: NewAccount,
    index: usize,
    total: usize,
    mut ask: impl FnMut(&str, &str) -> Input,
) -> Outcome {
    // 비밀키가 틀린 계정에 이름부터 묻지 않는다
    if let Err(e) = account.validate() {
        return Outcome::Failed(e);
    }
    // QR의 이름이 이미 쓰이고 있으면 첫 질문에서 바로 알린다. 빈 이름은 QR 쪽 사정이므로
    // 처음부터 나무라지 않고, 사용자가 비운 채 확인을 눌렀을 때 알린다.
    let mut problem = match state.check_name(&account.issuer, &account.name) {
        Ok(()) | Err(Error::InvalidName(_)) => None,
        Err(e @ Error::DuplicateName(_)) => Some(e),
        Err(e) => return Outcome::Failed(e),
    };
    let mut name = account.name.clone();
    loop {
        state.touch();
        let question = question(&account.issuer, index, total, problem.as_ref());
        match ask(&question, &name) {
            Input::Entered(text) => name = text.trim().to_string(),
            Input::Cancelled => return Outcome::Skipped,
            Input::Unavailable => {
                // 입력 상자를 띄울 수 없는 환경이다. 예전처럼 QR의 이름을 쓰되,
                // 겹치는 이름이면 등록하지 않는다.
                crate::log!("이름 입력 상자를 띄우지 못해 QR의 이름을 그대로 씁니다.");
                match problem {
                    Some(e) => return Outcome::Failed(e),
                    None => break,
                }
            }
        }
        state.touch();
        match state.check_name(&account.issuer, &name) {
            Ok(()) => break,
            Err(e @ (Error::DuplicateName(_) | Error::InvalidName(_))) => problem = Some(e),
            Err(e) => return Outcome::Failed(e),
        }
    }
    account.name = name;
    let shown = label(&account.issuer, &account.name);
    match state.add_manual(account, false) {
        Ok(_) => Outcome::Added(shown),
        Err(e) => Outcome::Failed(e),
    }
}

/// 입력 상자의 문구. 여러 계정을 가져올 때는 몇 번째인지, 앞선 입력이 틀렸으면 이유를 함께 쓴다.
fn question(issuer: &str, index: usize, total: usize, problem: Option<&Error>) -> String {
    let mut q = if total > 1 {
        format!("[{index}/{total}] 계정 이름을 정하십시오.")
    } else {
        "계정 이름을 정하십시오.".to_string()
    };
    if !issuer.is_empty() {
        q.push_str(&format!("\n발급자: {issuer}"));
    }
    match problem {
        Some(e @ Error::DuplicateName(_)) => {
            q.push_str(&format!("\n\n{e}\n다른 이름을 입력하십시오."));
        }
        Some(e) => q.push_str(&format!("\n\n{e}")),
        None => {}
    }
    q
}

#[cfg(test)]
mod tests {
    use super::*;
    use otpbar_core::crypto::KdfParams;
    use otpbar_core::{Algorithm, Vault};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// 시험용 금고를 연 상태. 임시 폴더는 Drop에서 지운다.
    struct Fixture {
        dir: std::path::PathBuf,
        state: AppState,
    }

    impl Fixture {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "otpbar-import-{tag}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let mut vault = Vault::new_at(dir.join("vault.json"));
            vault.set_kdf_params(KdfParams {
                m_cost: 8,
                t_cost: 1,
                p_cost: 1,
            });
            vault.create("correct horse battery").unwrap();
            let state = AppState::with_vault(vault);
            state.add_manual(account("bamin0422"), false).unwrap();
            Fixture { dir, state }
        }

        fn names(&self) -> Vec<String> {
            let mut names: Vec<String> = self
                .state
                .accounts()
                .unwrap()
                .into_iter()
                .map(|a| a.name)
                .collect();
            names.sort();
            names
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.dir).ok();
        }
    }

    fn account(name: &str) -> NewAccount {
        NewAccount {
            issuer: "authentik".into(),
            name: name.into(),
            secret: "JBSWY3DPEHPK3PXP".into(),
            algorithm: Algorithm::Sha1,
            digits: 6,
            period: 30,
        }
    }

    /// 가짜 입력 상자가 받은 (문구, 기본값) 기록.
    type Asked = Rc<RefCell<Vec<(String, String)>>>;

    /// 정해 둔 답을 차례로 내놓고, 받은 질문과 기본값을 기록하는 가짜 입력 상자.
    fn scripted(answers: Vec<Input>) -> (impl FnMut(&str, &str) -> Input, Asked) {
        let asked = Asked::default();
        let log = asked.clone();
        let mut answers = answers.into_iter();
        let ask = move |q: &str, d: &str| {
            log.borrow_mut().push((q.to_string(), d.to_string()));
            answers.next().expect("준비한 답보다 많이 물었다")
        };
        (ask, asked)
    }

    /// 핵심 흐름: QR의 이름이 이미 있으면 첫 질문에서 알리고, 같은 이름을 다시 넣어도
    /// 막으며, 다른 이름을 넣으면 등록한다. 예전처럼 말없이 건너뛰지 않는다.
    #[test]
    fn duplicate_name_is_reported_and_asked_again() {
        let fx = Fixture::new("dup");
        let (ask, asked) = scripted(vec![
            Input::Entered("bamin0422".into()),
            Input::Entered("  업무용  ".into()),
        ]);
        let outcome = register(&fx.state, account("bamin0422"), 1, 1, ask);
        assert!(matches!(outcome, Outcome::Added(ref n) if n == "authentik · 업무용"));
        assert_eq!(fx.names(), ["bamin0422", "업무용"]);

        let asked = asked.borrow();
        assert_eq!(asked.len(), 2);
        assert_eq!(asked[0].1, "bamin0422", "QR의 이름을 기본값으로 채운다");
        assert!(
            asked[0].0.contains("같은 이름의 계정이 이미 있습니다"),
            "첫 질문부터 겹친다는 사실을 알린다: {}",
            asked[0].0
        );
        assert!(asked[1].0.contains("같은 이름의 계정이 이미 있습니다"));
    }

    #[test]
    fn unique_name_is_registered_with_default() {
        let fx = Fixture::new("unique");
        let (ask, asked) = scripted(vec![Input::Entered("second".into())]);
        let outcome = register(&fx.state, account("second"), 2, 3, ask);
        assert!(matches!(outcome, Outcome::Added(_)));
        assert_eq!(fx.names(), ["bamin0422", "second"]);
        let asked = asked.borrow();
        assert!(asked[0].0.starts_with("[2/3] "));
        assert!(!asked[0].0.contains("이미 있습니다"));
    }

    #[test]
    fn empty_name_is_rejected_then_accepted() {
        let fx = Fixture::new("empty");
        let (ask, asked) = scripted(vec![
            Input::Entered("   ".into()),
            Input::Entered("third".into()),
        ]);
        let outcome = register(&fx.state, account("third"), 1, 1, ask);
        assert!(matches!(outcome, Outcome::Added(_)));
        assert!(asked.borrow()[1].0.contains("이름을 비울 수 없습니다"));
        assert_eq!(fx.names(), ["bamin0422", "third"]);
    }

    #[test]
    fn cancel_skips_without_adding() {
        let fx = Fixture::new("cancel");
        let (ask, _) = scripted(vec![Input::Cancelled]);
        let outcome = register(&fx.state, account("bamin0422"), 1, 1, ask);
        assert!(matches!(outcome, Outcome::Skipped));
        assert_eq!(fx.names(), ["bamin0422"]);
    }

    #[test]
    fn bad_secret_fails_without_asking() {
        let fx = Fixture::new("badsecret");
        let mut bad = account("x");
        bad.secret = "not base32!".into();
        let (ask, asked) = scripted(vec![]);
        let outcome = register(&fx.state, bad, 1, 1, ask);
        assert!(matches!(outcome, Outcome::Failed(_)));
        assert!(asked.borrow().is_empty());
    }

    /// 입력 상자를 띄울 수 없으면 예전처럼 QR의 이름을 쓴다. 겹치면 등록하지 않는다.
    #[test]
    fn without_dialog_falls_back_to_qr_name() {
        let fx = Fixture::new("nodialog");
        let (ask, _) = scripted(vec![Input::Unavailable]);
        assert!(matches!(
            register(&fx.state, account("fresh"), 1, 1, ask),
            Outcome::Added(_)
        ));
        let (ask, _) = scripted(vec![Input::Unavailable]);
        assert!(matches!(
            register(&fx.state, account("bamin0422"), 1, 1, ask),
            Outcome::Failed(Error::DuplicateName(_))
        ));
        assert_eq!(fx.names(), ["bamin0422", "fresh"]);
    }

    #[test]
    fn question_explains_duplicate() {
        let q = question(
            "GitHub",
            2,
            3,
            Some(&Error::DuplicateName("GitHub · admin".into())),
        );
        assert!(q.starts_with("[2/3] "));
        assert!(q.contains("발급자: GitHub"));
        assert!(q.contains("같은 이름의 계정이 이미 있습니다: GitHub · admin"));
        assert!(q.contains("다른 이름을 입력하십시오"));
    }

    #[test]
    fn question_for_single_account_is_plain() {
        let q = question("", 1, 1, None);
        assert_eq!(q, "계정 이름을 정하십시오.");
    }
}
