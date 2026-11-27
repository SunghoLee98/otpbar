//! 글자를 받아야 할 때만 잠깐 띄우는 시스템 다이얼로그.
//!
//! 이 앱에는 상주 창이 없다. 마스터 암호처럼 입력이 필요한 순간에만 운영체제의 표준
//! 입력 상자를 띄우고, 값을 받으면 곧바로 닫는다.
//!
//! 암호 입력 상자의 문구는 앱이 정한 고정 문자열이며, 사용자 입력이 스크립트로
//! 되돌아가는 경로는 없다(값은 표준 출력으로만 받는다).
//!
//! 글자 입력 상자([`text`])는 QR에서 읽은 발급자·이름을 문구와 기본값으로 보여 준다.
//! 이 글자는 믿을 수 없는 입력이므로 스크립트 본문에 끼워 넣지 않는다. macOS는
//! osascript의 인자로, Windows는 환경변수로 넘기고, 스크립트는 그것을 값으로만 읽는다.

/// [`text`]의 결과.
#[derive(Debug)]
pub enum Input {
    /// 확인을 눌렀다. 빈 글자일 수 있다.
    Entered(String),
    /// 취소를 눌렀다.
    Cancelled,
    /// 입력 상자를 띄울 수 없는 환경이다(리눅스에 zenity·kdialog가 없는 경우 등).
    Unavailable,
}

/// 기본값을 채운 입력 상자로 글자를 받는다. `ok`·`cancel`은 단추 이름이다.
pub fn text(prompt: &str, default: &str, ok: &str, cancel: &str) -> Input {
    #[cfg(target_os = "macos")]
    {
        macos_text(prompt, default, ok, cancel)
    }
    #[cfg(target_os = "windows")]
    {
        windows_text(prompt, default, ok, cancel)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        linux_text(prompt, default, ok, cancel)
    }
}

/// 표준 출력으로 받은 값에서 끝의 줄바꿈과, 혹시 붙었을 BOM을 걷어 낸다.
fn entered(stdout: &[u8]) -> Input {
    let text = String::from_utf8_lossy(stdout);
    Input::Entered(
        text.trim_start_matches('\u{feff}')
            .trim_end_matches(['\n', '\r'])
            .to_string(),
    )
}

/// 가림 입력으로 암호를 받는다. 취소하면 `None`.
pub fn password(prompt: &str) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        macos_password(prompt)
    }
    #[cfg(target_os = "windows")]
    {
        windows_password(prompt)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        linux_password(prompt)
    }
}

fn sanitize(prompt: &str) -> String {
    prompt.replace(['"', '\\', '\n', '\r'], " ")
}

#[cfg(target_os = "macos")]
fn macos_password(prompt: &str) -> Option<String> {
    let script = format!(
        r#"display dialog "{}" default answer "" with hidden answer with title "OTPBar" buttons {{"취소","확인"}} default button "확인" cancel button "취소""#,
        sanitize(prompt)
    );
    let out = std::process::Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .ok()?;
    if !out.status.success() {
        return None; // 취소
    }
    let text = String::from_utf8_lossy(&out.stdout);
    // "button returned:확인, text returned:비밀번호"
    let marker = "text returned:";
    let idx = text.find(marker)?;
    Some(
        text[idx + marker.len()..]
            .trim_end_matches(['\n', '\r'])
            .to_string(),
    )
}

#[cfg(target_os = "macos")]
fn macos_text(prompt: &str, default: &str, ok: &str, cancel: &str) -> Input {
    // 문구·기본값·단추 이름은 모두 run 핸들러의 인자(argv)로 받는다.
    // 취소 단추를 `cancel button`으로 지정해야 눌렀을 때 오류(-128)로 끝나 취소로 구분된다.
    const SCRIPT: [&str; 4] = [
        "on run argv",
        "set r to display dialog (item 1 of argv) default answer (item 2 of argv) \
         with title \"OTPBar\" buttons {item 4 of argv, item 3 of argv} \
         default button 2 cancel button 1",
        "return text returned of r",
        "end run",
    ];
    let mut cmd = std::process::Command::new("/usr/bin/osascript");
    for line in SCRIPT {
        cmd.arg("-e").arg(line);
    }
    match cmd.args([prompt, default, ok, cancel]).output() {
        Ok(out) if out.status.success() => entered(&out.stdout),
        Ok(_) => Input::Cancelled,
        Err(_) => Input::Unavailable,
    }
}

#[cfg(target_os = "windows")]
fn windows_password(prompt: &str) -> Option<String> {
    // WPF 창 하나로 가림 입력을 받는다. 값은 표준 출력으로만 나온다.
    let script = format!(
        r#"
Add-Type -AssemblyName PresentationFramework
$w = New-Object Windows.Window
$w.Title = 'OTPBar'; $w.Width = 380; $w.Height = 170
$w.WindowStartupLocation = 'CenterScreen'; $w.Topmost = $true; $w.ResizeMode = 'NoResize'
$sp = New-Object Windows.Controls.StackPanel; $sp.Margin = '16'
$tb = New-Object Windows.Controls.TextBlock; $tb.Text = "{}"; $tb.Margin = '0,0,0,10'; $tb.TextWrapping = 'Wrap'
$pb = New-Object Windows.Controls.PasswordBox; $pb.Margin = '0,0,0,12'; $pb.MinWidth = 320
$row = New-Object Windows.Controls.StackPanel; $row.Orientation = 'Horizontal'; $row.HorizontalAlignment = 'Right'
$ok = New-Object Windows.Controls.Button; $ok.Content = '확인'; $ok.Width = 80; $ok.Margin = '0,0,8,0'; $ok.IsDefault = $true
$cancel = New-Object Windows.Controls.Button; $cancel.Content = '취소'; $cancel.Width = 80; $cancel.IsCancel = $true
$ok.Add_Click({{ $w.DialogResult = $true }})
$row.AddChild($ok); $row.AddChild($cancel)
$sp.AddChild($tb); $sp.AddChild($pb); $sp.AddChild($row); $w.Content = $sp
$w.Add_ContentRendered({{ $pb.Focus() }})
if ($w.ShowDialog()) {{ [Console]::Out.Write($pb.Password) }}
"#,
        sanitize(prompt)
    );
    let sys32 = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    let ps = format!(r"{sys32}\System32\WindowsPowerShell\v1.0\powershell.exe");
    let out = std::process::Command::new(ps)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-STA",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &script,
        ])
        .output()
        .ok()?;
    let value = String::from_utf8_lossy(&out.stdout).to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

#[cfg(target_os = "windows")]
fn windows_text(prompt: &str, default: &str, ok: &str, cancel: &str) -> Input {
    // 값은 환경변수로 넘긴다. PowerShell은 `$env:`를 문자열 값으로만 읽고 해석하지 않는다.
    // 출력은 BOM 없는 UTF-8로 내보내야 한글 이름이 깨지지 않는다(기본은 OEM 코드 페이지).
    // 취소와 빈 입력을 구분하려고 종료 코드를 쓴다.
    const SCRIPT: &str = r#"
[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding $false
Add-Type -AssemblyName PresentationFramework
$w = New-Object Windows.Window
$w.Title = 'OTPBar'; $w.Width = 420; $w.SizeToContent = 'Height'
$w.WindowStartupLocation = 'CenterScreen'; $w.Topmost = $true; $w.ResizeMode = 'NoResize'
$sp = New-Object Windows.Controls.StackPanel; $sp.Margin = '16'
$tb = New-Object Windows.Controls.TextBlock; $tb.Text = $env:OTPBAR_PROMPT; $tb.Margin = '0,0,0,10'; $tb.TextWrapping = 'Wrap'
$box = New-Object Windows.Controls.TextBox; $box.Text = $env:OTPBAR_DEFAULT; $box.Margin = '0,0,0,12'; $box.MinWidth = 360
$row = New-Object Windows.Controls.StackPanel; $row.Orientation = 'Horizontal'; $row.HorizontalAlignment = 'Right'
$ok = New-Object Windows.Controls.Button; $ok.Content = $env:OTPBAR_OK; $ok.MinWidth = 80; $ok.Margin = '0,0,8,0'; $ok.IsDefault = $true
$cancel = New-Object Windows.Controls.Button; $cancel.Content = $env:OTPBAR_CANCEL; $cancel.MinWidth = 80; $cancel.IsCancel = $true
$ok.Add_Click({ $w.DialogResult = $true })
$row.AddChild($ok); $row.AddChild($cancel)
$sp.AddChild($tb); $sp.AddChild($box); $sp.AddChild($row); $w.Content = $sp
$w.Add_ContentRendered({ $box.Focus(); $box.SelectAll() })
if ($w.ShowDialog()) { [Console]::Out.Write($box.Text); exit 0 }
exit 1
"#;
    let sys32 = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    let ps = format!(r"{sys32}\System32\WindowsPowerShell\v1.0\powershell.exe");
    let out = std::process::Command::new(ps)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-STA",
            "-WindowStyle",
            "Hidden",
            "-Command",
            SCRIPT,
        ])
        .env("OTPBAR_PROMPT", prompt)
        .env("OTPBAR_DEFAULT", default)
        .env("OTPBAR_OK", ok)
        .env("OTPBAR_CANCEL", cancel)
        .output();
    match out {
        Ok(out) if out.status.success() => entered(&out.stdout),
        Ok(_) => Input::Cancelled,
        Err(_) => Input::Unavailable,
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn linux_password(prompt: &str) -> Option<String> {
    for (bin, args) in [
        (
            "zenity",
            vec![
                "--password".to_string(),
                format!("--title={}", sanitize(prompt)),
            ],
        ),
        ("kdialog", vec!["--password".to_string(), sanitize(prompt)]),
    ] {
        if let Ok(out) = std::process::Command::new(bin).args(&args).output() {
            if out.status.success() {
                return Some(String::from_utf8_lossy(&out.stdout).trim_end().to_string());
            }
        }
    }
    None
}

#[cfg(all(unix, not(target_os = "macos")))]
fn linux_text(prompt: &str, default: &str, ok: &str, cancel: &str) -> Input {
    // 셸을 거치지 않으므로 인자는 그대로 전달된다. `--entry`의 문구는 마크업으로
    // 해석되지 않는다(zenity 4에서 확인).
    let zenity = std::process::Command::new("zenity")
        .arg("--entry")
        .arg("--title=OTPBar")
        .arg(format!("--text={prompt}"))
        .arg(format!("--entry-text={default}"))
        .arg(format!("--ok-label={ok}"))
        .arg(format!("--cancel-label={cancel}"))
        .output();
    match zenity {
        Ok(out) if out.status.success() => return entered(&out.stdout),
        Ok(_) => return Input::Cancelled,
        Err(_) => {} // 설치되어 있지 않다. kdialog를 시도한다.
    }
    let kdialog = std::process::Command::new("kdialog")
        .args(["--title", "OTPBar", "--inputbox", prompt, default])
        .output();
    match kdialog {
        Ok(out) if out.status.success() => entered(&out.stdout),
        Ok(_) => Input::Cancelled,
        Err(_) => Input::Unavailable,
    }
}
