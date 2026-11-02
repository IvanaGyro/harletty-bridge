//! Real TrueHD through the session: an Atmos stream and a 5.1 one, downloaded
//! by CI (never committed; see `.github/workflows`). Point
//! `HARLETTY_MF_TRUEHD_DIR` at a folder holding
//!
//! - `atmos.thd`: https://fate-suite.ffmpeg.org/truehd/atmos.thd
//! - `ffvii.mkv`: "TrueHD Sample 1 (Final Fantasy VII Advent Children).mkv",
//!   from https://samples.ffmpeg.org/A-codecs/TrueHD/. Its audio blocks are
//!   fed one by one with their timestamps, as a player's demuxer would.
//!
//! Without the variable the tests say so and pass, so a plain `cargo test`
//! needs no download; CI also sets `HARLETTY_MF_REQUIRE_TRUEHD=1`, which makes
//! a missing file a failure.

use std::path::{Path, PathBuf};

use harletty_mf::portable::codecs::{CODECS, Codec};
use harletty_mf::portable::session::{Block, Session, Setup};

const STEREO: u32 = 0x3;
/// 7.1.4: FL FR C LFE BL BR SL SR TFL TFR TBL TBR.
const SEVEN_ONE_FOUR: u32 = 0x63F | 0x1000 | 0x4000 | 0x8000 | 0x20000;

fn truehd() -> &'static Codec {
    CODECS.iter().find(|c| c.option == "truehd").unwrap()
}

/// The sample `name`, or `None` (a skip) when no folder was given.
fn sample(name: &str) -> Option<Vec<u8>> {
    let Some(dir) = std::env::var_os("HARLETTY_MF_TRUEHD_DIR").map(PathBuf::from) else {
        assert!(
            std::env::var_os("HARLETTY_MF_REQUIRE_TRUEHD").is_none(),
            "HARLETTY_MF_REQUIRE_TRUEHD is set but HARLETTY_MF_TRUEHD_DIR is not"
        );
        eprintln!("skipped: HARLETTY_MF_TRUEHD_DIR is not set");
        return None;
    };
    Some(std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}")))
}

/// One Matroska block of the audio track: its payload and its time.
struct Packet {
    data: Vec<u8>,
    pts_hns: i64,
}

/// The vint at `at`: (value, length). `None` for an unknown size (all ones).
fn vint(bytes: &[u8], at: usize, keep_marker: bool) -> Option<(Option<u64>, usize)> {
    let first = *bytes.get(at)?;
    let len = first.leading_zeros() as usize + 1;
    if len > 8 || at + len > bytes.len() {
        return None;
    }
    let mut value = if keep_marker {
        first as u64
    } else {
        (first as u64) & ((1u64 << (8 - len)) - 1)
    };
    let mut ones = value == (1u64 << (8 - len)) - 1;
    for &b in &bytes[at + 1..at + len] {
        value = (value << 8) | b as u64;
        ones &= b == 0xFF;
    }
    Some((
        if ones && !keep_marker {
            None
        } else {
            Some(value)
        },
        len,
    ))
}

/// The audio track's blocks, in file order. A flat scan: the master elements
/// that matter are stepped into, everything else is skipped by its size, so
/// unknown-size segments and clusters need no special case. Lacing is not
/// handled (TrueHD in Matroska is not laced).
fn mkv_audio_packets(bytes: &[u8]) -> Vec<Packet> {
    const MASTERS: [u64; 5] = [0x18538067, 0x1F43B675, 0xA0, 0x1654AE6B, 0xAE];
    let (mut at, mut scale_ns, mut cluster_ms) = (0usize, 1_000_000u64, 0u64);
    let (mut entry_number, mut entry_audio, mut audio_track) = (0u64, false, None);
    let mut packets = Vec::new();
    while at < bytes.len() {
        let Some((Some(id), id_len)) = vint(bytes, at, true) else {
            break;
        };
        let Some((size, size_len)) = vint(bytes, at + id_len, false) else {
            break;
        };
        let body = at + id_len + size_len;
        if MASTERS.contains(&id) {
            if id == 0xAE {
                (entry_number, entry_audio) = (0, false);
            }
            at = body;
            continue;
        }
        let end = body + size.unwrap_or(0) as usize;
        let payload = bytes.get(body..end.min(bytes.len())).unwrap_or(&[]);
        let uint = |p: &[u8]| p.iter().fold(0u64, |v, &b| (v << 8) | b as u64);
        match id {
            0x2AD7B1 => scale_ns = uint(payload),
            // Track number and type may come in either order.
            0xD7 | 0x83 => {
                if id == 0xD7 {
                    entry_number = uint(payload);
                } else {
                    entry_audio = uint(payload) == 2;
                }
                if entry_audio && entry_number != 0 {
                    audio_track = Some(entry_number);
                }
            }
            0xE7 => cluster_ms = uint(payload),
            0xA3 | 0xA1 => {
                if let Some((Some(number), n)) = vint(payload, 0, false) {
                    if Some(number) == audio_track && payload.len() > n + 3 {
                        let rel = i16::from_be_bytes([payload[n], payload[n + 1]]) as i64;
                        let ticks = cluster_ms as i64 + rel;
                        packets.push(Packet {
                            data: payload[n + 3..].to_vec(),
                            pts_hns: ticks * scale_ns as i64 / 100,
                        });
                    }
                }
            }
            _ => {}
        }
        at = end;
    }
    packets
}

fn setup(tag: &str) -> Setup {
    Setup {
        work_dir: std::env::temp_dir().join(format!("harletty-mf-{tag}-{}", std::process::id())),
        engine_config: None,
    }
}

fn decode(session: &mut Session, stream: &[u8], size: usize) -> Vec<Block> {
    let mut blocks = Vec::new();
    for piece in stream.chunks(size) {
        blocks.extend(session.push(piece, None).expect("push"));
    }
    blocks.extend(session.drain().expect("drain"));
    blocks
}

fn frames(blocks: &[Block]) -> usize {
    blocks.iter().map(|b| b.frames).sum()
}

fn peak(blocks: &[Block]) -> f32 {
    blocks
        .iter()
        .flat_map(|b| b.samples.iter())
        .fold(0.0, |m, &v| m.max(v.abs()))
}

/// Decode `name` to `mask` and check what any real stream must satisfy; returns
/// the blocks for further checks.
fn plays(name: &str, mask: u32, tag: &str) -> Option<Vec<Block>> {
    let stream = sample(name)?;
    let mut session = Session::open(truehd(), 48_000, mask, &setup(tag)).expect("open");
    let blocks = decode(&mut session, &stream, 4096);
    let seconds = frames(&blocks) as f64 / 48_000.0;
    eprintln!(
        "{name} -> 0x{mask:X}: {} blocks, {seconds:.2} s, peak {}",
        blocks.len(),
        peak(&blocks)
    );
    assert!(!blocks.is_empty(), "{name}: no output");
    for b in &blocks {
        assert_eq!(b.samples.len(), b.frames * session.channels() as usize);
    }
    for pair in blocks.windows(2) {
        assert_eq!(pair[0].start_hns + pair[0].duration_hns, pair[1].start_hns);
    }
    assert!(peak(&blocks) > 1e-3, "{name}: silent output");
    assert!(peak(&blocks) < 8.0, "{name}: absurd peak {}", peak(&blocks));
    Some(blocks)
}

#[test]
fn an_atmos_stream_renders_to_seven_one_four() {
    plays("atmos.thd", SEVEN_ONE_FOUR, "atmos-714");
}

#[test]
fn an_atmos_stream_renders_binaurally_to_stereo() {
    plays("atmos.thd", STEREO, "atmos-stereo");
}

/// The Matroska sample, packet by packet with timestamps.
fn plays_mkv(mask: u32, tag: &str) -> Option<(Vec<Packet>, Vec<Block>)> {
    let packets = mkv_audio_packets(&sample("ffvii.mkv")?);
    assert!(packets.len() > 100, "found {} audio blocks", packets.len());
    let mut session = Session::open(truehd(), 48_000, mask, &setup(tag)).expect("open");
    let mut blocks = Vec::new();
    for p in &packets {
        blocks.extend(session.push(&p.data, Some(p.pts_hns)).expect("push"));
    }
    blocks.extend(session.drain().expect("drain"));
    let seconds = frames(&blocks) as f64 / 48_000.0;
    eprintln!(
        "ffvii.mkv -> 0x{mask:X}: {} packets, {} blocks, {seconds:.2} s, peak {}",
        packets.len(),
        blocks.len(),
        peak(&blocks)
    );
    // Every packet is stamped and each stamp re-anchors the clock (plan,
    // "Timestamps"), while Matroska stamps are whole milliseconds: a block may
    // start up to a millisecond off where the last one ended. Not more.
    for pair in blocks.windows(2) {
        let gap = pair[1].start_hns - (pair[0].start_hns + pair[0].duration_hns);
        assert!(gap.abs() <= 10_000, "gap of {gap} x 100 ns between blocks");
    }
    assert!(peak(&blocks) > 1e-3, "silent output");
    // The sample is 16.18 s long.
    assert!((15.5..=16.5).contains(&seconds), "{seconds} s");
    Some((packets, blocks))
}

#[test]
fn a_five_one_track_plays_through_its_whole_length_to_seven_one_four() {
    plays_mkv(SEVEN_ONE_FOUR, "ffvii-714");
}

#[test]
fn a_five_one_track_renders_binaurally_to_stereo_on_the_timeline() {
    if let Some((packets, blocks)) = plays_mkv(STEREO, "ffvii-stereo") {
        // Output starts where the first packet does, and ends near the last.
        let first = packets[0].pts_hns;
        assert!(
            (blocks[0].start_hns - first).abs() < 2_000_000,
            "starts off"
        );
        let last = blocks.last().unwrap();
        let end = last.start_hns + last.duration_hns;
        assert!(
            (end - packets.last().unwrap().pts_hns).abs() < 5_000_000,
            "ends off"
        );
    }
}

#[test]
fn the_linked_in_decoder_equals_the_bridge_dll_on_real_truehd() {
    let exe = std::env::current_exe().unwrap();
    let bridges: &Path = exe.parent().unwrap();
    for name in ["atmos.thd", "ffvii.mkv"] {
        let Some(mut stream) = sample(name) else {
            return;
        };
        if name.ends_with(".mkv") {
            // The audio payloads back to back, as one raw stream.
            stream = mkv_audio_packets(&stream)
                .into_iter()
                .flat_map(|p| p.data)
                .collect();
        }
        let mut linked = Session::open(truehd(), 48_000, STEREO, &setup("eq-a")).unwrap();
        let mut reference =
            Session::open_reference(truehd(), 48_000, STEREO, &setup("eq-b"), bridges).unwrap();
        assert_eq!(
            decode(&mut linked, &stream, 4096),
            decode(&mut reference, &stream, 4096),
            "{name}: the two sessions differ"
        );
    }
}
