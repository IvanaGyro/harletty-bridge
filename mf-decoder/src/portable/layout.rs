//! The speaker layout the engine renders to: one speaker per bit of a Windows
//! channel mask (`SPEAKER_*`), in ascending bit order, which is also the order
//! of the interleaved channels in a `WAVEFORMATEXTENSIBLE` stream. The engine's
//! output then needs no reordering.
//!
//! Coordinates follow Omniphony's layouts (`layouts/7.1.4.yaml`): x to the
//! right, y to the front, z up, normalised to a radius of 1; the LFE is not
//! spatialised. The names are `canonical_name`s of the channel labels, which
//! `bridge_api::labels::label_for_name` maps back to the same label.

use bridge_api::RChannelLabel;
use bridge_api::labels::canonical_name;

pub const SPEAKER_FRONT_LEFT: u32 = 0x1;
pub const SPEAKER_FRONT_RIGHT: u32 = 0x2;
/// What a mask with no speaker we place falls back to.
pub const STEREO: u32 = SPEAKER_FRONT_LEFT | SPEAKER_FRONT_RIGHT;

/// One speaker of a layout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Speaker {
    /// The `SPEAKER_*` bit.
    pub bit: u32,
    pub label: RChannelLabel,
    pub position: [f32; 3],
    pub spatialize: bool,
}

const fn speaker(bit: u32, label: RChannelLabel, position: [f32; 3]) -> Speaker {
    Speaker {
        bit,
        label,
        position,
        spatialize: true,
    }
}

/// Every `SPEAKER_*` bit we place, ascending. `SPEAKER_TOP_BACK_CENTER`
/// (0x10000) has no label in `bridge_api`, so it is left out of the mask.
const SPEAKERS: &[Speaker] = &[
    speaker(0x1, RChannelLabel::L, [-1.0, 1.0, 0.0]),
    speaker(0x2, RChannelLabel::R, [1.0, 1.0, 0.0]),
    speaker(0x4, RChannelLabel::C, [0.0, 1.0, 0.0]),
    Speaker {
        bit: 0x8,
        label: RChannelLabel::LFE,
        position: [1.0, 1.0, -1.0],
        spatialize: false,
    },
    speaker(0x10, RChannelLabel::Lb, [-1.0, -1.0, 0.0]),
    speaker(0x20, RChannelLabel::Rb, [1.0, -1.0, 0.0]),
    speaker(0x40, RChannelLabel::Lsc, [-0.5, 1.0, 0.0]),
    speaker(0x80, RChannelLabel::Rsc, [0.5, 1.0, 0.0]),
    speaker(0x100, RChannelLabel::Cb, [0.0, -1.0, 0.0]),
    speaker(0x200, RChannelLabel::Ls, [-1.0, 0.0, 0.0]),
    speaker(0x400, RChannelLabel::Rs, [1.0, 0.0, 0.0]),
    speaker(0x800, RChannelLabel::Tc, [0.0, 0.0, 1.0]),
    speaker(0x1000, RChannelLabel::Tfl, [-1.0, 1.0, 1.0]),
    speaker(0x2000, RChannelLabel::Tfc, [0.0, 1.0, 1.0]),
    speaker(0x4000, RChannelLabel::Tfr, [1.0, 1.0, 1.0]),
    speaker(0x8000, RChannelLabel::Tbl, [-1.0, -1.0, 1.0]),
    speaker(0x20000, RChannelLabel::Tbr, [1.0, -1.0, 1.0]),
];

/// The part of `mask` we can place, or stereo when none of it is.
pub fn effective_mask(mask: u32) -> u32 {
    let placed = SPEAKERS.iter().fold(0, |acc, s| acc | (mask & s.bit));
    if placed == 0 { STEREO } else { placed }
}

/// The speakers of `mask`, ascending by bit (see [`effective_mask`]).
pub fn speakers(mask: u32) -> Vec<Speaker> {
    let mask = effective_mask(mask);
    SPEAKERS
        .iter()
        .filter(|s| mask & s.bit != 0)
        .copied()
        .collect()
}

/// 7.1.4, the layout rendered to virtually when the real one is too small.
pub const SEVEN_ONE_FOUR: u32 = 0x63F | 0x1000 | 0x4000 | 0x8000 | 0x20000;

/// What the engine renders for an output mask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plan {
    /// The channels the output carries: the mask, or stereo when binaural.
    pub output_mask: u32,
    /// The speaker layout the engine pans to.
    pub render_mask: u32,
    /// Fold the rendered layout down to two channels through an HRTF. The
    /// engine's panner needs at least three speakers, so stereo (and mono)
    /// output is rendered binaurally from a full layout.
    pub binaural: bool,
}

/// How to render for `mask` (see [`Plan`]).
pub fn plan(mask: u32) -> Plan {
    let mask = effective_mask(mask);
    if mask.count_ones() >= 3 {
        Plan {
            output_mask: mask,
            render_mask: mask,
            binaural: false,
        }
    } else {
        Plan {
            output_mask: STEREO,
            render_mask: SEVEN_ONE_FOUR,
            binaural: true,
        }
    }
}

/// The engine configuration that switches on the binaural stage.
pub const BINAURAL_CONFIG: &str = "render:\n  binaural:\n    output_mode: binaural\n";

/// The channel labels of `mask`'s speakers, in output order.
pub fn labels(mask: u32) -> Vec<RChannelLabel> {
    speakers(mask).iter().map(|s| s.label).collect()
}

/// The layout as the YAML file `Engine::from_paths` reads.
pub fn layout_yaml(mask: u32) -> String {
    let mask = effective_mask(mask);
    let mut yaml = format!("name: \"harletty-mf 0x{mask:X}\"\nradius_m: 1\nspeakers:\n");
    for s in speakers(mask) {
        let [x, y, z] = s.position;
        yaml.push_str(&format!(
            "  - name: \"{}\"\n    coord_mode: \"cartesian\"\n    x: {x:.1}\n    y: {y:.1}\n    z: {z:.1}\n    spatialize: {}\n    delay_ms: 0\n",
            canonical_name(s.label),
            s.spatialize
        ));
    }
    yaml
}

#[cfg(test)]
mod tests {
    use super::*;
    use RChannelLabel::*;
    use bridge_api::labels::label_for_name;

    #[test]
    fn names_map_back_to_their_labels() {
        for s in SPEAKERS {
            assert_eq!(label_for_name(canonical_name(s.label)), s.label);
        }
    }

    #[test]
    fn speakers_ascend_by_bit() {
        assert!(SPEAKERS.windows(2).all(|pair| pair[0].bit < pair[1].bit));
    }

    #[test]
    fn seven_one_is_in_waveformat_order() {
        // FL FR FC LFE BL BR SL SR
        let labels: Vec<_> = speakers(0x63F).iter().map(|s| s.label).collect();
        assert_eq!(labels, [L, R, C, LFE, Lb, Rb, Ls, Rs]);
    }

    #[test]
    fn seven_one_four_matches_the_omniphony_layout() {
        // layouts/7.1.4.yaml, in mask order: FL FR C LFE BL BR SL SR TFL TFR TBL TBR
        let mask = 0x63F | 0x1000 | 0x4000 | 0x8000 | 0x20000;
        let labels: Vec<_> = speakers(mask).iter().map(|s| s.label).collect();
        assert_eq!(labels, [L, R, C, LFE, Lb, Rb, Ls, Rs, Tfl, Tfr, Tbl, Tbr]);
    }

    #[test]
    fn nothing_we_place_falls_back_to_stereo() {
        assert_eq!(effective_mask(0), STEREO);
        assert_eq!(effective_mask(0x10000), STEREO); // top back centre only
        assert_eq!(effective_mask(0x10000 | 0x4), 0x4);
    }

    #[test]
    fn two_or_fewer_speakers_render_binaurally_from_a_full_layout() {
        assert_eq!(
            plan(0x3),
            Plan {
                output_mask: STEREO,
                render_mask: SEVEN_ONE_FOUR,
                binaural: true
            }
        );
        assert_eq!(plan(0x4).output_mask, STEREO); // mono output is given stereo
        assert_eq!(plan(0).binaural, true);
        let five_one = plan(0x3F);
        assert_eq!(
            (
                five_one.output_mask,
                five_one.render_mask,
                five_one.binaural
            ),
            (0x3F, 0x3F, false)
        );
        assert_eq!(plan(0x7).output_mask, 0x7); // three is enough for the panner
    }

    #[test]
    fn only_the_lfe_is_not_spatialised() {
        let lfe: Vec<_> = SPEAKERS
            .iter()
            .filter(|s| !s.spatialize)
            .map(|s| s.label)
            .collect();
        assert_eq!(lfe, [LFE]);
    }

    #[test]
    fn the_yaml_parses_and_has_one_entry_per_speaker() {
        let yaml = layout_yaml(0x63F);
        let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&yaml).unwrap();
        let list = value["speakers"].as_sequence().unwrap();
        assert_eq!(list.len(), 8);
        assert_eq!(list[3]["name"].as_str(), Some("LFE"));
        assert_eq!(list[3]["spatialize"].as_bool(), Some(false));
        assert_eq!(list[0]["x"].as_f64(), Some(-1.0));
    }
}
