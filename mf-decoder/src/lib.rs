//! harletty-bridge as a Windows Media Foundation audio decoder
//! (`harletty_mf.dll`): the plan is docs/plan-media-foundation-decoder.md.
//!
//! [`portable`] is everything that is not COM and builds and tests on every
//! platform; `com` (Windows only) is the thin `IMFTransform` shell around it.
//! (The module is not called `core`: that would shadow the `core` crate.)

pub mod portable;

#[cfg(windows)]
pub mod com;
