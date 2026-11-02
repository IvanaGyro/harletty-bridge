//! The panic path (plan Phase 3, step 3): a panic inside the decoder comes back
//! as `E_UNEXPECTED`, the next sample decodes, and the process lives. The
//! panic is injected by a hook that exists only in the unit-test build.

#[path = "../../tests/common/mod.rs"]
mod common;

use std::sync::atomic::Ordering;

use common::*;
use windows::Win32::Foundation::E_UNEXPECTED;
use windows::Win32::Media::MediaFoundation::MFT_MESSAGE_COMMAND_FLUSH;

use crate::portable::session::test_hooks::PANIC_NEXT_PUSH;

#[test]
fn a_panic_is_e_unexpected_and_the_next_sample_decodes() {
    let mft = decoder();
    start(&mft);
    let first = &DTS_CORE[..PIECE];

    PANIC_NEXT_PUSH.store(true, Ordering::SeqCst);
    let err = unsafe { mft.ProcessInput(0, &access_unit(first, Some(0), false), 0) }
        .expect_err("the injected panic must surface as an error");
    assert_eq!(err.code(), E_UNEXPECTED);
    assert!(!PANIC_NEXT_PUSH.load(Ordering::SeqCst), "the hook fired");

    // The transform is usable: flush, and the whole stream decodes.
    unsafe { mft.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0).unwrap() };
    assert!(!decode_all(&mft).is_empty(), "no audio after the panic");
}
