use std::fs;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::fs::{atomic_write, config_path};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    pub agent: AgentConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AgentConfig {
    pub backend: String,
    pub output: AgentOutputMode,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum AgentOutputMode {
    Hidden,
    Format,
    Full,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            agent: AgentConfig {
                backend: "codex".to_string(),
                output: AgentOutputMode::Format,
            },
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }

        let bytes = fs::read(path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path()?;
        let mut bytes = serde_json::to_vec_pretty(self)?;
        bytes.push(b'\n');
        atomic_write(&path, &bytes)
    }

    pub fn print(&self) -> Result<()> {
        let text = serde_json::to_string_pretty(self)?;
        println!("{text}");
        Ok(())
    }
}
