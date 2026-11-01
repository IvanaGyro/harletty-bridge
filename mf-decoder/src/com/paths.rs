//! Where the DLL and its per-user files live.

use std::path::PathBuf;

use windows::Win32::Foundation::{HMODULE, MAX_PATH};
use windows::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
    GetModuleFileNameW, GetModuleHandleExW,
};
use windows::core::PCWSTR;

/// This DLL's own file, found from the address of a function in it (not by
/// name, and not by the process's search path: in `wmplayer.exe` that starts
/// in WMP's folder).
pub fn dll_file() -> windows::core::Result<PathBuf> {
    unsafe {
        let mut module = HMODULE::default();
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            PCWSTR(dll_file as *const () as *const u16),
            &mut module,
        )?;
        let mut buffer = vec![0u16; MAX_PATH as usize * 4];
        let len = GetModuleFileNameW(Some(module), &mut buffer) as usize;
        if len == 0 {
            return Err(windows::core::Error::from_thread());
        }
        Ok(PathBuf::from(String::from_utf16_lossy(&buffer[..len])))
    }
}

/// The folder holding this DLL (and, later, the bridge DLLs and `mf.yaml`).
pub fn dll_dir() -> Option<PathBuf> {
    dll_file().ok()?.parent().map(PathBuf::from)
}

/// `%LOCALAPPDATA%\harletty\mf`: per-user options and the log.
pub fn user_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|base| PathBuf::from(base).join("harletty").join("mf"))
}

/// `mf.yaml`, from next to the DLL and then the user folder.
pub fn load_config() -> crate::portable::config::MfConfig {
    let dll = dll_dir();
    let user = user_dir();
    let folders: Vec<&std::path::Path> = [dll.as_deref(), user.as_deref()]
        .into_iter()
        .flatten()
        .collect();
    crate::portable::config::MfConfig::load(&folders)
}
