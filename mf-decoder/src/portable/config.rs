//! `mf.yaml`: the decoder's own options, looked up next to the DLL and then in
//! the per-user folder, which wins key by key. The engine's configuration
//! (`config.yaml`) is a separate file in the Omniphony schema.

use std::path::Path;

use serde::Deserialize;

use super::codecs::Codec;

pub const FILE_NAME: &str = "mf.yaml";

/// The speaker mask as written: a number or `0x…` text.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
enum Mask {
    Number(u32),
    Text(String),
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MfConfig {
    /// Overrides the default audio endpoint's speaker mask.
    speaker_mask: Option<Mask>,
    /// The codecs to register, by `option` name (`truehd`, `dts`, `eac3`,
    /// `ac3`); the codec table's defaults when absent.
    pub codecs: Option<Vec<String>>,
    /// `off` … `trace`; `warn` when absent.
    pub log_level: Option<String>,
    /// Where to log; `%LOCALAPPDATA%\harletty\mf\harletty-mf.log` when absent.
    pub log_file: Option<String>,
}

impl MfConfig {
    pub fn parse(text: &str) -> Result<Self, String> {
        serde_yaml_ng::from_str::<Option<Self>>(text)
            .map(Option::unwrap_or_default)
            .map_err(|e| e.to_string())
    }

    /// The speaker mask override, if the file has one.
    pub fn speaker_mask(&self) -> Result<Option<u32>, String> {
        match &self.speaker_mask {
            None => Ok(None),
            Some(Mask::Number(n)) => Ok(Some(*n)),
            Some(Mask::Text(text)) => {
                let digits = text.trim();
                let parsed = match digits
                    .strip_prefix("0x")
                    .or_else(|| digits.strip_prefix("0X"))
                {
                    Some(hex) => u32::from_str_radix(hex, 16),
                    None => digits.parse(),
                };
                parsed
                    .map(Some)
                    .map_err(|_| format!("speaker_mask {text:?} is not a number"))
            }
        }
    }

    /// Whether `codec` is switched on.
    pub fn enabled(&self, codec: &Codec) -> bool {
        match &self.codecs {
            None => codec.default,
            Some(list) => list
                .iter()
                .any(|name| name.eq_ignore_ascii_case(codec.option)),
        }
    }

    /// `other`'s keys win where it sets them.
    fn overlay(mut self, other: MfConfig) -> Self {
        if other.speaker_mask.is_some() {
            self.speaker_mask = other.speaker_mask;
        }
        if other.codecs.is_some() {
            self.codecs = other.codecs;
        }
        if other.log_level.is_some() {
            self.log_level = other.log_level;
        }
        if other.log_file.is_some() {
            self.log_file = other.log_file;
        }
        self
    }

    /// Read [`FILE_NAME`] from each folder in turn, later ones winning. A
    /// missing file is nothing; one that does not parse is logged and skipped,
    /// so a typo cannot stop the decoder from loading.
    pub fn load(folders: &[&Path]) -> Self {
        let mut config = MfConfig::default();
        for folder in folders {
            let path = folder.join(FILE_NAME);
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            match Self::parse(&text) {
                Ok(file) => config = config.overlay(file),
                Err(e) => log::warn!("{}: {e}; ignored", path.display()),
            }
        }
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::portable::codecs::CODECS;

    #[test]
    fn empty_and_missing_keys_are_defaults() {
        assert_eq!(MfConfig::parse("").unwrap(), MfConfig::default());
        assert_eq!(
            MfConfig::parse("log_level: debug")
                .unwrap()
                .log_level
                .as_deref(),
            Some("debug")
        );
    }

    #[test]
    fn the_mask_is_a_number_or_hex_text() {
        let mask = |text: &str| MfConfig::parse(text).unwrap().speaker_mask();
        assert_eq!(mask("speaker_mask: 63").unwrap(), Some(63));
        assert_eq!(mask("speaker_mask: 0x63F").unwrap(), Some(0x63F));
        assert_eq!(mask("speaker_mask: \"0x3\"").unwrap(), Some(3));
        assert_eq!(mask("log_level: warn").unwrap(), None);
        assert!(mask("speaker_mask: lots").is_err());
    }

    #[test]
    fn unknown_keys_are_refused() {
        assert!(MfConfig::parse("speakers: 8").is_err());
    }

    #[test]
    fn codecs_default_to_the_table_and_can_be_listed() {
        let default = MfConfig::default();
        let names = |config: &MfConfig| -> Vec<&str> {
            CODECS
                .iter()
                .filter(|c| config.enabled(c))
                .map(|c| c.option)
                .collect()
        };
        assert_eq!(names(&default), ["truehd", "dts", "dts"]);
        let listed = MfConfig::parse("codecs: [TrueHD, eac3]").unwrap();
        assert_eq!(names(&listed), ["truehd", "eac3"]);
    }

    #[test]
    fn the_user_folder_overrides_key_by_key() {
        let base = std::env::temp_dir().join(format!("harletty-mf-config-{}", std::process::id()));
        let (dll, user) = (base.join("dll"), base.join("user"));
        std::fs::create_dir_all(&dll).unwrap();
        std::fs::create_dir_all(&user).unwrap();
        std::fs::write(dll.join(FILE_NAME), "log_level: info\nspeaker_mask: 3\n").unwrap();
        std::fs::write(user.join(FILE_NAME), "log_level: trace\n").unwrap();
        let config = MfConfig::load(&[&dll, &user]);
        assert_eq!(config.log_level.as_deref(), Some("trace"));
        assert_eq!(config.speaker_mask().unwrap(), Some(3));
        // A file that does not parse changes nothing.
        std::fs::write(user.join(FILE_NAME), "log_level: [\n").unwrap();
        assert_eq!(
            MfConfig::load(&[&dll, &user]).log_level.as_deref(),
            Some("info")
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
