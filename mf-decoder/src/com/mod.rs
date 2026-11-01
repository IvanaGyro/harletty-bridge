//! The COM shell (Windows only): a class factory, the `IMFTransform`, the DLL
//! exports and the registration, around [`crate::portable`].

mod endpoint;
mod exports;
mod factory;
mod logger;
mod paths;
mod registry;
mod transform;

pub use exports::{DllCanUnloadNow, DllGetClassObject, DllRegisterServer, DllUnregisterServer};
use windows::core::GUID;

/// The decoder's class id.
pub const CLSID_DECODER: GUID = GUID::from_u128(0xAC884ED4_9379_4E5A_BAF6_3365A4F08E38);
/// The name `MFTRegister` and the CLSID key carry.
pub const FRIENDLY_NAME: &str = "harletty decoder (TrueHD, DTS)";
