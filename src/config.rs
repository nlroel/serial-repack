use std::collections::HashSet;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config: {0}")]
    Read(#[from] std::io::Error),
    #[error("failed to parse TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("no enabled channels configured")]
    NoEnabledChannels,
    #[error("duplicate channel name: {0}")]
    DuplicateChannel(String),
    #[error("channel {channel} has invalid hex in {field}: {source}")]
    InvalidHex {
        channel: String,
        field: &'static str,
        source: hex::FromHexError,
    },
    #[error(
        "channel {channel} packet_len {packet_len} is smaller than header+tail length {min_len}"
    )]
    PacketTooShort {
        channel: String,
        packet_len: usize,
        min_len: usize,
    },
    #[error("channel {channel} has unsupported serial setting {field}={value}")]
    UnsupportedSerialSetting {
        channel: String,
        field: &'static str,
        value: String,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Config {
    pub channels: Vec<ChannelConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ChannelConfig {
    pub name: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub serial: SerialConfig,
    pub packet: PacketConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct SerialConfig {
    pub port: String,
    #[serde(default)]
    pub passthrough_port: Option<String>,
    pub baud_rate: u32,
    #[serde(default = "default_data_bits")]
    pub data_bits: u8,
    #[serde(default = "default_stop_bits")]
    pub stop_bits: u8,
    #[serde(default = "default_parity")]
    pub parity: String,
    #[serde(default = "default_flow_control")]
    pub flow_control: String,
    #[serde(default = "default_read_timeout_ms")]
    pub read_timeout_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct PacketConfig {
    pub packet_len: usize,
    pub header: String,
    pub tail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedChannel {
    pub name: String,
    pub serial: SerialConfig,
    pub packet_len: usize,
    pub header: Vec<u8>,
    pub tail: Vec<u8>,
}

fn default_enabled() -> bool {
    true
}

fn default_data_bits() -> u8 {
    8
}

fn default_stop_bits() -> u8 {
    1
}

fn default_parity() -> String {
    "none".to_string()
}

fn default_flow_control() -> String {
    "none".to_string()
}

fn default_read_timeout_ms() -> u64 {
    100
}

impl Config {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let text = fs::read_to_string(path)?;
        Self::from_toml_str(&text)
    }

    pub fn from_toml_str(text: &str) -> Result<Self, ConfigError> {
        let config: Config = toml::from_str(text)?;
        config.validate()?;
        Ok(config)
    }

    pub fn enabled_channels(&self) -> Vec<&ChannelConfig> {
        self.channels.iter().filter(|ch| ch.enabled).collect()
    }

    pub fn validated_channels(&self) -> Result<Vec<ValidatedChannel>, ConfigError> {
        self.validate()?;
        self.enabled_channels()
            .into_iter()
            .map(|ch| {
                let header = decode_hex(&ch.name, "header", &ch.packet.header)?;
                let tail = decode_hex(&ch.name, "tail", &ch.packet.tail)?;
                Ok(ValidatedChannel {
                    name: ch.name.clone(),
                    serial: ch.serial.clone(),
                    packet_len: ch.packet.packet_len,
                    header,
                    tail,
                })
            })
            .collect()
    }

    fn validate(&self) -> Result<(), ConfigError> {
        let mut names = HashSet::new();
        let mut enabled = 0usize;

        for channel in &self.channels {
            if !names.insert(channel.name.clone()) {
                return Err(ConfigError::DuplicateChannel(channel.name.clone()));
            }

            validate_serial(&channel.name, &channel.serial)?;

            let header = decode_hex(&channel.name, "header", &channel.packet.header)?;
            let tail = decode_hex(&channel.name, "tail", &channel.packet.tail)?;
            let min_len = header.len() + tail.len();
            if channel.packet.packet_len < min_len {
                return Err(ConfigError::PacketTooShort {
                    channel: channel.name.clone(),
                    packet_len: channel.packet.packet_len,
                    min_len,
                });
            }

            if channel.enabled {
                enabled += 1;
            }
        }

        if enabled == 0 {
            return Err(ConfigError::NoEnabledChannels);
        }

        Ok(())
    }
}

fn decode_hex(channel: &str, field: &'static str, value: &str) -> Result<Vec<u8>, ConfigError> {
    hex::decode(value).map_err(|source| ConfigError::InvalidHex {
        channel: channel.to_string(),
        field,
        source,
    })
}

fn validate_serial(channel: &str, serial: &SerialConfig) -> Result<(), ConfigError> {
    if !matches!(serial.data_bits, 5..=8) {
        return Err(ConfigError::UnsupportedSerialSetting {
            channel: channel.to_string(),
            field: "data_bits",
            value: serial.data_bits.to_string(),
        });
    }
    if !matches!(serial.stop_bits, 1 | 2) {
        return Err(ConfigError::UnsupportedSerialSetting {
            channel: channel.to_string(),
            field: "stop_bits",
            value: serial.stop_bits.to_string(),
        });
    }
    if !matches!(serial.parity.as_str(), "none" | "odd" | "even") {
        return Err(ConfigError::UnsupportedSerialSetting {
            channel: channel.to_string(),
            field: "parity",
            value: serial.parity.clone(),
        });
    }
    if !matches!(
        serial.flow_control.as_str(),
        "none" | "software" | "hardware"
    ) {
        return Err(ConfigError::UnsupportedSerialSetting {
            channel: channel.to_string(),
            field: "flow_control",
            value: serial.flow_control.clone(),
        });
    }
    if serial.read_timeout_ms == 0 {
        return Err(ConfigError::UnsupportedSerialSetting {
            channel: channel.to_string(),
            field: "read_timeout_ms",
            value: serial.read_timeout_ms.to_string(),
        });
    }
    Ok(())
}
