//! One line per event in `%PUBLIC%\harletty-mf-stub.log`: time, PID,
//! executable, whether the process is an AppContainer, and the event. A failed
//! write is ignored: a sandboxed process may not be allowed to write there, and
//! that is itself a finding (the plan says to use `tasklist /m` then).

use std::io::Write;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{GetTokenInformation, TOKEN_QUERY, TokenIsAppContainer};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

fn is_app_container() -> Option<bool> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).ok()?;
        let mut value = 0u32;
        let mut returned = 0u32;
        let result = GetTokenInformation(
            token,
            TokenIsAppContainer,
            Some(&mut value as *mut u32 as *mut _),
            size_of::<u32>() as u32,
            &mut returned,
        );
        let _ = CloseHandle(token);
        result.ok()?;
        Some(value != 0)
    }
}

pub fn line(event: &str) {
    let Some(dir) = std::env::var_os("PUBLIC") else {
        return;
    };
    let path = std::path::Path::new(&dir).join("harletty-mf-stub.log");
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    else {
        return;
    };
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "?".into());
    let container = match is_app_container() {
        Some(true) => "appcontainer",
        Some(false) => "desktop",
        None => "unknown",
    };
    let _ = writeln!(
        file,
        "{secs} pid={} {container} exe={exe} {event}",
        std::process::id()
    );
}
