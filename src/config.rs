use lazy_static::lazy_static;
use serde::{de::Error, Deserialize, Deserializer, Serialize};
use std::path::PathBuf;

lazy_static! {
    static ref IDENTIFIER_REGEX: regex::Regex = regex::Regex::new("[a-zA-Z0-9_]+").unwrap();
}

const fn default_mqtt_port() -> u16 {
    1883
}
const fn default_sunspec_port() -> u16 {
    502
}
fn default_client_id() -> String {
    "gpio2mqtt_bridge".to_owned()
}

#[derive(Deserialize)]
pub struct Config {
    #[serde(default = "default_client_id")]
    pub client_id: String,

    pub covers: Option<Vec<CoverConfig>>,
    pub sunspec_devices: Option<Vec<SunspecConfig>>,
    pub host: String,

    #[serde(default = "default_mqtt_port")]
    pub port: u16,

    pub global_tx_timeout_ms: u64,
}

#[derive(Serialize, Debug)]
pub struct Identifier(pub String);

#[derive(Serialize, Deserialize, Debug)]
pub struct Device {
    pub identifier: Identifier,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub sw_version: Option<String>,
}

impl<'de> Deserialize<'de> for Identifier {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let id = String::deserialize(de)?;

        if IDENTIFIER_REGEX.is_match(&id) {
            Ok(Identifier(id))
        } else {
            Err(Error::custom("identifier must match [a-zA-Z0-9_]+"))
        }
    }
}

#[derive(Deserialize)]
pub struct CoverConfig {
    pub name: String,
    pub chip: PathBuf,
    pub up_pin: u32,
    pub down_pin: u32,
    pub stop_pin: u32,
    pub tx_timeout_ms: Option<u64>,
    pub device: Device,
}

#[derive(Deserialize)]
pub struct SunspecConfig {
    pub name: String,
    pub device: Device,
    pub host: String,

    #[serde(default = "default_sunspec_port")]
    pub port: u16,
}
