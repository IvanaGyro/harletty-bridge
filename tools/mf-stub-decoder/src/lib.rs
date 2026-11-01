//! Phase 0 stub (docs/plan-media-foundation-decoder.md, "Phase 0", step 4.1):
//! a Media Foundation audio decoder that plays silence, and writes one line to
//! `%PUBLIC%\harletty-mf-stub.log` whenever a process loads it, sets an input
//! type or asks for output. It decodes nothing. It exists to find out which
//! players load a decoder registered machine-wide with `regsvr32`.
//!
//! Registered input types: TrueHD, DTS-HD and DTS; with
//! `HARLETTY_STUB_EAC3=1` in the environment of `regsvr32`, also E-AC-3 and
//! AC-3. Output: 32-bit float PCM, 48 kHz, stereo.

#![cfg(windows)]

mod log;
mod registration;
mod transform;

use std::ffi::c_void;
use windows::Win32::Foundation::{
    CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_NOINTERFACE, E_POINTER, S_FALSE, S_OK,
};
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};
use windows::core::{BOOL, GUID, HRESULT, IUnknown, Interface, Ref, implement};

/// The stub's class id. The real decoder has its own, so the two never collide.
pub const CLSID_STUB: GUID = GUID::from_u128(0x7CB8888B_36DA_4A32_B2F9_99748D739782);

#[implement(IClassFactory)]
struct Factory;

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<IUnknown>,
        iid: *const GUID,
        object: *mut *mut c_void,
    ) -> windows::core::Result<()> {
        if object.is_null() || iid.is_null() {
            return Err(E_POINTER.into());
        }
        unsafe { *object = std::ptr::null_mut() };
        if !outer.is_null() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let unknown: IUnknown = transform::new().into();
        let hr = unsafe { unknown.query(iid, object) };
        if hr.is_err() {
            return Err(E_NOINTERFACE.into());
        }
        Ok(())
    }

    fn LockServer(&self, _lock: BOOL) -> windows::core::Result<()> {
        Ok(())
    }
}

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
    log::line("DllGetClassObject");
    if unsafe { *clsid } != CLSID_STUB {
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    let factory: IClassFactory = Factory.into();
    unsafe { factory.query(iid, object) }
}

/// Always `S_FALSE`: the stub stays loaded for as long as the process lives,
/// which is what the log is there to show.
#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    S_FALSE
}

#[unsafe(no_mangle)]
pub extern "system" fn DllRegisterServer() -> HRESULT {
    match registration::register() {
        Ok(()) => S_OK,
        Err(e) => e.code(),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn DllUnregisterServer() -> HRESULT {
    match registration::unregister() {
        Ok(()) => S_OK,
        Err(e) => e.code(),
    }
}
