use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_theme")]
    pub theme: String,
}
fn default_theme() -> String {
    "terminal".into()
}
impl Default for Config {
    fn default() -> Self {
        Self {
            theme: default_theme(),
        }
    }
}
impl Config {
    pub fn data_dir() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("config directory unavailable"))?
            .join("local-review");
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }
    pub fn load() -> Result<Self> {
        let file = Self::data_dir()?.join("config.toml");
        if !file.exists() {
            return Ok(Self::default());
        }
        Ok(toml::from_str(&fs::read_to_string(file)?)?)
    }
    pub fn save_theme(theme: &str) -> Result<()> {
        let file = Self::data_dir()?.join("config.toml");
        fs::write(
            file,
            toml::to_string_pretty(&Self {
                theme: theme.to_owned(),
            })?,
        )?;
        Ok(())
    }
}
