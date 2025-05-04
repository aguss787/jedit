use std::{fs::File, io::Read};

use byte_unit::{Byte, Unit};
use serde::Deserialize;

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Config {
    pub max_preview_size: Byte,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_preview_size: Byte::from_u64_with_unit(1, Unit::MiB)
                .expect("failed to build default max_preview_size"),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        Self::default().patch_from_files(&["/etc/jedit", "~/.jedit", ".jedit"])
    }

    fn patch_from_files(self, files: &[&str]) -> Self {
        files
            .iter()
            .map(File::open)
            .filter_map(Result::ok)
            .filter_map(|mut file| {
                let mut content = String::new();
                file.read_to_string(&mut content).ok()?;
                Some(content)
            })
            .filter_map(|content| toml::from_str(&content).ok())
            .fold(self, Self::patch)
    }

    fn patch(mut self, patch: ConfigPatch) -> Self {
        if let Some(max_preview_size) = patch.max_preview_size {
            self.max_preview_size = max_preview_size
        }

        self
    }
}

#[cfg(test)]
impl Config {
    pub fn with_max_preview_size(mut self, max_preview_size: Byte) -> Self {
        self.max_preview_size = max_preview_size;
        self
    }
}

#[derive(Debug, Default, Deserialize)]
#[cfg_attr(test, derive(serde::Serialize))]
struct ConfigPatch {
    pub max_preview_size: Option<Byte>,
}

#[cfg(test)]
mod test {
    use std::io::Write;

    use super::*;

    #[test]
    fn config_patch_test() {
        let config = Config::default();
        let patch = ConfigPatch {
            max_preview_size: None,
        };

        let config = config.patch(patch);
        assert_eq!(config, Config::default());

        let patch = ConfigPatch {
            max_preview_size: Some(Byte::from_u64(123)),
        };
        let config = config.patch(patch);
        assert_eq!(
            config,
            Config::default().with_max_preview_size(Byte::from_u64(123))
        );
    }

    #[test]
    fn config_patch_from_files() {
        setup_file("/tmp/jedit-config-bogus", "bogus");
        let config = Config::default().patch_from_files(&["/tmp/jedit-config-bogus"]);
        assert_eq!(config, Config::default());

        setup_file(
            "/tmp/jedit-config-none",
            &toml::to_string_pretty(&ConfigPatch {
                max_preview_size: None,
            })
            .unwrap(),
        );
        let config = Config::default().patch_from_files(&["/tmp/jedit-config-none"]);
        assert_eq!(config, Config::default());

        setup_file(
            "/tmp/jedit-config-some",
            &toml::to_string_pretty(&ConfigPatch {
                max_preview_size: Some(Byte::from_u64(123)),
            })
            .unwrap(),
        );
        let config = Config::default().patch_from_files(&["/tmp/jedit-config-some"]);
        assert_eq!(
            config,
            Config::default().with_max_preview_size(Byte::from_u64(123))
        );

        setup_file(
            "/tmp/jedit-config-some-2",
            &toml::to_string_pretty(&ConfigPatch {
                max_preview_size: Some(Byte::from_u64(1234)),
            })
            .unwrap(),
        );
        let config = Config::default()
            .patch_from_files(&["/tmp/jedit-config-some", "/tmp/jedit-config-some-2"]);
        assert_eq!(
            config,
            Config::default().with_max_preview_size(Byte::from_u64(1234))
        );
    }

    fn setup_file(file_path: &str, content: &str) {
        let mut file = File::create(file_path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
    }
}
