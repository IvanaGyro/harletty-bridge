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

/// A deterministic stream of pseudo-random numbers, so a failure reproduces.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
}

/// Feed `stream` to a fresh session in 1 KiB pieces, ignoring what comes out:
/// the point is only that it returns. Errors are fine (a host reports them and
/// goes on); a panic or a hang is not.
fn survives(option: &str, stream: Vec<u8>, label: &str) {
    let (done, result) = std::sync::mpsc::channel();
    let option = option.to_owned();
    let tag = label.to_owned();
    std::thread::spawn(move || {
        let outcome = std::panic::catch_unwind(|| {
            let mut session =
                Session::open(codec(&option), 48_000, STEREO, &setup(&tag)).expect("open");
            for piece in stream.chunks(1024) {
                // The transform resets after an error; so does this.
                if session.push(piece, None).is_err() {
                    session.reset();
                }
            }
            let _ = session.drain();
        });
        let _ = done.send(outcome.is_ok());
    });
    match result.recv_timeout(std::time::Duration::from_secs(120)) {
        Ok(true) => {}
        Ok(false) => panic!("{label}: the session panicked"),
        Err(_) => panic!("{label}: the session did not return"),
    }
}

#[test]
fn corrupted_and_truncated_streams_never_panic_or_hang() {
    for (option, fixture) in [("eac3", EAC3), ("dts", DTS)] {
        let mut rng = Lcg(0x5EED);
        // Truncated anywhere, including mid-frame.
        for cut in [1, 7, fixture.len() / 3, fixture.len() / 2 + 1] {
            survives(
                option,
                fixture[..cut].to_vec(),
                &format!("{option}-cut{cut}"),
            );
        }
        // Bits flipped: a few, many, and in the header of the first frame.
        for (n, label) in [(1, "one"), (16, "few"), (fixture.len() / 8, "many")] {
            let mut bytes = fixture.to_vec();
            for _ in 0..n {
                let at = rng.next() as usize % bytes.len();
                bytes[at] ^= 1 << (rng.next() % 8);
            }
            survives(option, bytes, &format!("{option}-flip-{label}"));
        }
        let mut head = fixture.to_vec();
        for byte in head.iter_mut().take(16) {
            *byte ^= 0xFF;
        }
        survives(option, head, &format!("{option}-head"));
        // Garbage, and nothing sensible at all.
        let garbage: Vec<u8> = (0..64 * 1024).map(|_| rng.next() as u8).collect();
        survives(option, garbage, &format!("{option}-garbage"));
        survives(option, vec![0; 64 * 1024], &format!("{option}-zeros"));
        survives(option, vec![0xFF; 64 * 1024], &format!("{option}-ones"));
    }
}

/// A session whose engine is configured by `yaml` (Omniphony's `config.yaml`
/// schema), the way a user's `mf-config.yaml` configures it.
fn configured(option: &str, mask: u32, yaml: &str, tag: &str) -> Session {
    let mut setup = setup(tag);
    std::fs::create_dir_all(&setup.work_dir).unwrap();
    let path = setup.work_dir.join("mf-config.yaml");
    std::fs::write(&path, yaml).unwrap();
    setup.engine_config = Some(path);
    Session::open(codec(option), 48_000, mask, &setup).unwrap_or_else(|e| panic!("open: {e}"))
}

fn render(session: &mut Session, stream: &[u8]) -> Vec<Block> {
    decode(session, stream, 4 * 1024)
}

#[test]
fn the_engine_takes_drc_from_the_config() {
    let plain = render(
        &mut Session::open(codec("eac3"), 48_000, STEREO, &setup("drc-plain")).unwrap(),
        EAC3,
    );
    let off = render(
        &mut configured("eac3", STEREO, "render:\n  drc_mode: Off\n", "drc-off"),
        EAC3,
    );
    let heavy = render(
        &mut configured(
            "eac3",
            STEREO,
            "render:\n  drc_mode: heavy/RF\n",
            "drc-heavy",
        ),
        EAC3,
    );
    // Off is the default; heavy compression changes the signal.
    assert_eq!(plain, off, "drc_mode: Off is not the default");
    assert_eq!(frames(&heavy), frames(&off));
    assert_ne!(heavy, off, "drc_mode: heavy/RF changed nothing");
}

#[test]
fn the_engine_takes_the_presentation_from_the_config() {
    // A substream the stream does not have must not stop playback: the bridge
    // keeps its default (the engine warns), and the audio is the default's.
    let plain = render(
        &mut Session::open(codec("eac3"), 48_000, STEREO, &setup("pres-plain")).unwrap(),
        EAC3,
    );
    let asked = render(
        &mut configured("eac3", STEREO, "render:\n  presentation: 3\n", "pres-3"),
        EAC3,
    );
    assert!(!asked.is_empty());
    assert_eq!(frames(&plain), frames(&asked));
}

#[test]
fn a_config_that_does_not_parse_means_the_defaults_not_a_failure() {
    let plain = render(
        &mut Session::open(codec("dts"), 48_000, STEREO, &setup("cfg-plain")).unwrap(),
        DTS,
    );
    let broken = render(
        &mut configured(
            "dts",
            STEREO,
            "render: [this is: not\n  a mapping",
            "cfg-broken",
        ),
        DTS,
    );
    assert_eq!(plain, broken);
}
