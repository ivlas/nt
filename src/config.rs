use std::fs;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::fs::{atomic_write, nt_home};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    pub agent: AgentConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AgentConfig {
    #[serde(default = "default_agent_output")]
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
                output: default_agent_output(),
            },
        }
    }
}

fn default_agent_output() -> AgentOutputMode {
    AgentOutputMode::Format
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }

        let text = fs::read_to_string(path)?;
        Ok(toml::from_str(&text)?)
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path()?;
        let mut text = toml::to_string_pretty(self)?;
        text.push('\n');
        atomic_write(&path, text.as_bytes())
    }

    pub fn print(&self) -> Result<()> {
        let text = toml::to_string_pretty(self)?;
        print!("{text}");
        if !text.ends_with('\n') {
            println!();
        }
        Ok(())
    }
}

fn config_path() -> Result<std::path::PathBuf> {
    Ok(nt_home()?.join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::{AgentOutputMode, Config};

    #[test]
    fn config_serializes_as_toml() {
        let config = Config::default();
        let text = toml::to_string_pretty(&config).unwrap();

        assert!(text.contains("[agent]"));
        assert!(text.contains("output = \"format\""));
        assert!(!text.contains("backend"));

        let parsed: Config = toml::from_str(&text).unwrap();
        assert_eq!(parsed.agent.output, AgentOutputMode::Format);
    }

    #[test]
    fn config_defaults_missing_agent_output() {
        let parsed: Config = toml::from_str("[agent]\n").unwrap();

        assert_eq!(parsed.agent.output, AgentOutputMode::Format);
    }
}
