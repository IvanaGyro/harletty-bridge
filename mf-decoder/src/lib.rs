//! harletty-bridge as a Windows Media Foundation audio decoder
//! (`harletty_mf.dll`): the plan is docs/plan-media-foundation-decoder.md.
//!
//! [`portable`] is everything that is not COM and builds and tests on every
//! platform; `com` (Windows only) is the thin `IMFTransform` shell around it.
//! (The module is not called `core`: that would shadow the `core` crate.)

// The DLL exports DllGetClassObject & co. for COM to find, which the MSVC
// linker's LNK4104 says "should be PRIVATE" (they are only meant for a .def
// file). They are exports on purpose; the warning is not about anything else.
#![allow(linker_messages)]

// The unit tests include the COM tests' helpers (tests/common), which name this
// crate as an outside user does.
#[cfg(test)]
extern crate self as harletty_mf;

pub mod portable;

#[cfg(windows)]
pub mod com;
