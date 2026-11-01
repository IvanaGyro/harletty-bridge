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

/// Where the bridge DLLs are: `HARLETTY_MF_BRIDGE_DIR` (for tests and
/// troubleshooting), else `bridge_dir` in `mf.yaml`, else this DLL's folder.
pub fn bridge_dir(config: &crate::portable::config::MfConfig) -> Option<PathBuf> {
    std::env::var_os("HARLETTY_MF_BRIDGE_DIR")
        .map(PathBuf::from)
        .or_else(|| config.bridge_dir.as_ref().map(PathBuf::from))
        .or_else(dll_dir)
}

/// An engine configuration of the user's own (`mf-config.yaml`, in Omniphony's
/// `config.yaml` schema), from the per-user folder and then next to the DLL.
/// Not Omniphony's shared config: the engine consumes a live-handoff file
/// beside whatever config it opens, which would take Studio's.
pub fn engine_config() -> Option<PathBuf> {
    [user_dir(), dll_dir()]
        .into_iter()
        .flatten()
        .map(|dir| dir.join("mf-config.yaml"))
        .find(|path| path.is_file())
}

/// Everything a session needs to find, or why it cannot.
pub fn session_setup(
    config: &crate::portable::config::MfConfig,
) -> Result<crate::portable::session::Setup, String> {
    let bridge_dir = bridge_dir(config).ok_or("cannot find the folder of the DLL")?;
    let work_dir = user_dir().unwrap_or_else(|| std::env::temp_dir().join("harletty-mf"));
    Ok(crate::portable::session::Setup {
        bridge_dir,
        work_dir,
        engine_config: engine_config(),
    })
}
