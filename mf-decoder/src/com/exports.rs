//! The four exports COM and `regsvr32` look for.

use std::ffi::c_void;

use windows::Win32::Foundation::{CLASS_E_CLASSNOTAVAILABLE, E_POINTER, S_FALSE, S_OK};
use windows::Win32::System::Com::IClassFactory;
use windows::core::{GUID, HRESULT, Interface};

use super::CLSID_DECODER;

/// # Safety
/// Called by COM with valid pointers or null.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetClassObject(
    clsid: *const GUID,
    iid: *const GUID,
    object: *mut *mut c_void,
) -> HRESULT {
    if clsid.is_null() || iid.is_null() || object.is_null() {
        return E_POINTER;
    }
    // The first sign that a process is using the DLL: start the log here, so
    // it exists even if nothing goes further.
    super::logger::init(&super::paths::load_config());
    log::info!(
        "DllGetClassObject in {:?}",
        std::env::current_exe()
            .ok()
            .map(|p| p.display().to_string())
    );
    if unsafe { *clsid } != CLSID_DECODER {
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    let factory: IClassFactory = super::factory::Factory.into();
    unsafe { factory.query(iid, object) }
}

/// Always `S_FALSE`: a decoder loaded into a player stays until the process
/// ends.
#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    S_FALSE
}

#[unsafe(no_mangle)]
pub extern "system" fn DllRegisterServer() -> HRESULT {
    match super::registry::register() {
        Ok(()) => S_OK,
        Err(e) => e.code(),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn DllUnregisterServer() -> HRESULT {
    match super::registry::unregister() {
        Ok(()) => S_OK,
        Err(e) => e.code(),
    }
}
