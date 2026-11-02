//! The input types the decoder takes: what Windows' Matroska source calls each
//! codec, and the `input_codec` name the bridges know it by.
//!
//! The subtype GUIDs are the ones Windows itself reports (the Phase 0 probe,
//! docs/media-foundation-findings.md): TrueHD and DTS-HD from `CodecQuery`
//! listing Windows' own converters under them, DTS as `MFAudioFormat_DTS`
//! (WAVE_FORMAT_DTS), E-AC-3 and AC-3 from the SDK.

/// One input type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Codec {
    /// For logs and `mf.yaml`.
    pub name: &'static str,
    /// The `MF_MT_SUBTYPE` GUID as a `u128` (`GUID::from_u128`).
    pub subtype: u128,
    /// What `configure("input_codec", …)` takes: one of the families'
    /// `INPUT_CODECS`.
    pub input_codec: &'static str,
    /// The `codecs:` name in `mf.yaml` that switches it on and off.
    pub option: &'static str,
    /// Which family decodes it: `dolby` or `dts`. (Linked in; the name is also
    /// the bridge library's, `harletty_<family>_bridge`, that the reference
    /// session in the tests loads.)
    pub bridge: &'static str,
    /// Registered unless `mf.yaml` says otherwise. E-AC-3 and AC-3 are not:
    /// Microsoft's decoder already plays them.
    pub default: bool,
}

pub const CODECS: &[Codec] = &[
    Codec {
        name: "Dolby TrueHD",
        subtype: 0xEB27CEC4_163E_4CA3_8B74_8E25F91B517E, // MEDIASUBTYPE_DOLBY_TRUEHD
        input_codec: "truehd",
        option: "truehd",
        bridge: "dolby",
        default: true,
    },
    Codec {
        name: "DTS-HD",
        subtype: 0xA2E58EB7_0FA9_48BB_A40C_FA0E156D0645, // MEDIASUBTYPE_DTS_HD
        input_codec: "dts",
        option: "dts",
        bridge: "dts",
        default: true,
    },
    Codec {
        name: "DTS",
        subtype: 0x00000008_0000_0010_8000_00AA00389B71, // MFAudioFormat_DTS, WAVE_FORMAT_DTS
        input_codec: "dts",
        option: "dts",
        bridge: "dts",
        default: true,
    },
    Codec {
        name: "Dolby Digital Plus",
        subtype: 0xA7FB87AF_2D02_42FB_A4D4_05CD93843BDD, // MFAudioFormat_Dolby_DDPlus
        input_codec: "eac3",
        option: "eac3",
        bridge: "dolby",
        default: false,
    },
    Codec {
        name: "Dolby Digital",
        subtype: 0xE06D802C_DB46_11CF_B4D1_00805F6CBBEA, // MFAudioFormat_Dolby_AC3
        input_codec: "ac3",
        option: "ac3",
        bridge: "dolby",
        default: false,
    },
];

/// The codec with this subtype, if the table has one.
pub fn find(subtype: u128) -> Option<&'static Codec> {
    CODECS.iter().find(|codec| codec.subtype == subtype)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bridge_common::family::FamilyPipeline;
    use bridge_family_dolby::DolbyPipeline;
    use bridge_family_dts::DtsPipeline;

    #[test]
    fn subtypes_are_unique() {
        for (i, a) in CODECS.iter().enumerate() {
            for b in &CODECS[i + 1..] {
                assert_ne!(a.subtype, b.subtype, "{} and {}", a.name, b.name);
            }
        }
    }

    #[test]
    fn every_input_codec_is_one_a_family_takes() {
        for codec in CODECS {
            let taken = DolbyPipeline::INPUT_CODECS.contains(&codec.input_codec)
                || DtsPipeline::INPUT_CODECS.contains(&codec.input_codec);
            assert!(
                taken,
                "{}: no family takes {:?}",
                codec.name, codec.input_codec
            );
        }
    }

    #[test]
    fn every_codec_names_a_bridge_that_lists_it() {
        for codec in CODECS {
            let family: &[&str] = match codec.bridge {
                "dolby" => DolbyPipeline::INPUT_CODECS,
                "dts" => DtsPipeline::INPUT_CODECS,
                other => panic!("{}: unknown bridge {other:?}", codec.name),
            };
            assert!(family.contains(&codec.input_codec), "{}", codec.name);
        }
    }

    #[test]
    fn find_by_subtype() {
        assert_eq!(
            find(CODECS[0].subtype).map(|c| c.name),
            Some("Dolby TrueHD")
        );
        assert!(find(0).is_none());
    }

    #[test]
    fn dolby_digital_is_opt_in() {
        let default: Vec<_> = CODECS
            .iter()
            .filter(|c| c.default)
            .map(|c| c.option)
            .collect();
        assert_eq!(default, ["truehd", "dts", "dts"]);
    }
}
