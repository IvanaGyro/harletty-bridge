//! The session on the real engine and the real bridge libraries, decoding the
//! repository's own fixtures (the E-AC-3 JOC and DTS ones the bridge tests use).
//!
//! The decoders are linked into the library (plan D10). The reference session
//! loads the same families as bridge DLLs through `Engine::from_paths`: the
//! cdylibs cargo builds because the plugin crates are dev-dependencies, next to
//! this test's executable.

use std::path::{Path, PathBuf};

use harletty_mf::portable::codecs::{CODECS, Codec};
use harletty_mf::portable::session::{Block, Session, Setup};

const EAC3: &[u8] = include_bytes!("../../harletty/tests/fixtures/joc_atmos_1s.eac3");
const DTS: &[u8] = include_bytes!("../../harletty/tests/fixtures/dts_core_tone_10f.dts");

const STEREO: u32 = 0x3;
/// 7.1.4: FL FR C LFE BL BR SL SR TFL TFR TBL TBR.
const SEVEN_ONE_FOUR: u32 = 0x63F | 0x1000 | 0x4000 | 0x8000 | 0x20000;

fn codec(option: &str) -> &'static Codec {
    CODECS.iter().find(|c| c.option == option).unwrap()
}

fn bridge_dir() -> PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn setup(tag: &str) -> Setup {
    let work = std::env::temp_dir().join(format!("harletty-mf-{tag}-{}", std::process::id()));
    Setup {
        work_dir: work,
        engine_config: None,
    }
}

/// The stream as a host feeds it: pieces of `size` bytes.
fn decode(session: &mut Session, stream: &[u8], size: usize) -> Vec<Block> {
    let mut blocks = Vec::new();
    for piece in stream.chunks(size) {
        blocks.extend(session.push(piece, None).expect("push"));
    }
    blocks.extend(session.drain().expect("drain"));
    blocks
}

fn peak(blocks: &[Block]) -> f32 {
    blocks
        .iter()
        .flat_map(|b| b.samples.iter())
        .fold(0.0, |m, &v| m.max(v.abs()))
}

fn frames(blocks: &[Block]) -> usize {
    blocks.iter().map(|b| b.frames).sum()
}

#[test]
fn joc_objects_render_to_a_seven_one_four_layout() {
    let mut session =
        Session::open(codec("eac3"), 48_000, SEVEN_ONE_FOUR, &setup("joc")).expect("open");
    assert_eq!((session.channels(), session.mask()), (12, SEVEN_ONE_FOUR));
    let blocks = decode(&mut session, EAC3, 64 * 1024);
    eprintln!(
        "joc: {} blocks, {} frames, peak {}",
        blocks.len(),
        frames(&blocks),
        peak(&blocks)
    );
    assert!(!blocks.is_empty());
    for b in &blocks {
        assert_eq!(b.samples.len(), b.frames * 12);
    }
    // About a second of audio, and not silence.
    let seconds = frames(&blocks) as f64 / 48_000.0;
    assert!((0.5..=2.0).contains(&seconds), "{seconds} s");
    assert!(peak(&blocks) > 1e-3, "silent output");
}

#[test]
fn blocks_tile_on_the_timeline() {
    let mut session = Session::open(codec("eac3"), 48_000, STEREO, &setup("tile")).expect("open");
    let blocks = decode(&mut session, EAC3, 64 * 1024);
    assert!(blocks.len() > 1);
    for pair in blocks.windows(2) {
        assert_eq!(pair[0].start_hns + pair[0].duration_hns, pair[1].start_hns);
    }
}

#[test]
fn the_same_stream_in_other_piece_sizes_decodes_the_same() {
    let mut a = Session::open(codec("eac3"), 48_000, STEREO, &setup("a")).expect("open");
    let mut b = Session::open(codec("eac3"), 48_000, STEREO, &setup("b")).expect("open");
    let one = decode(&mut a, EAC3, 64 * 1024);
    let other = decode(&mut b, EAC3, 4 * 1024);
    assert_eq!(frames(&one), frames(&other));
    assert_eq!(peak(&one), peak(&other));
}

#[test]
fn dts_core_decodes_to_stereo() {
    let mut session = Session::open(codec("dts"), 48_000, STEREO, &setup("dts")).expect("open");
    let blocks = decode(&mut session, DTS, 64 * 1024);
    eprintln!(
        "dts: {} blocks, {} frames, peak {}",
        blocks.len(),
        frames(&blocks),
        peak(&blocks)
    );
    assert!(frames(&blocks) > 0);
    assert!(peak(&blocks) > 1e-3, "silent output");
}

#[test]
fn reset_forgets_the_stream_but_not_the_setup() {
    let mut session = Session::open(codec("eac3"), 48_000, STEREO, &setup("reset")).expect("open");
    let first = decode(&mut session, EAC3, 64 * 1024);
    session.reset();
    let second = decode(&mut session, EAC3, 64 * 1024);
    assert_eq!(frames(&first), frames(&second));
    // The second pass starts over on the timeline.
    assert_eq!(second[0].start_hns, first[0].start_hns);
}

/// The linked-in session is the bridge-DLL session: same engine, same
/// output, bit for bit, on every fixture and every output.
#[test]
fn linked_in_decoders_equal_the_bridge_dlls() {
    let cases: [(&str, &[u8], u32); 4] = [
        ("eac3", EAC3, SEVEN_ONE_FOUR),
        ("eac3", EAC3, STEREO),
        ("dts", DTS, STEREO),
        ("dts", DTS, SEVEN_ONE_FOUR),
    ];
    for (option, stream, mask) in cases {
        let codec = codec(option);
        let mut linked = Session::open(codec, 48_000, mask, &setup("linked")).expect("linked");
        let mut reference =
            Session::open_reference(codec, 48_000, mask, &setup("ref"), &bridge_dir())
                .expect("reference");
        let (a, b) = (
            decode(&mut linked, stream, 4 * 1024),
            decode(&mut reference, stream, 4 * 1024),
        );
        assert!(!a.is_empty(), "{option} 0x{mask:X}: no output");
        assert_eq!(a, b, "{option} 0x{mask:X}: the two sessions differ");
    }
}

#[test]
fn a_missing_bridge_is_an_error_naming_the_file() {
    let err = Session::open_reference(
        codec("dts"),
        48_000,
        STEREO,
        &setup("missing"),
        Path::new("no-such-folder"),
    )
    .unwrap_err();
    assert!(
        err.contains("not found") && err.contains("harletty_dts_bridge"),
        "{err}"
    );
}
