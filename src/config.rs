use regex::Regex;
use serde::{de::Error, Deserialize, Deserializer, Serialize};
use std::{path::PathBuf, sync::OnceLock};

static IDENTIFIER_REGEX: OnceLock<Regex> = OnceLock::new();

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

    pub covers: Option<Vec<CoverGroup>>,
    pub sunspec: Option<Vec<SunspecConfig>>,

    pub broker: String,

    #[serde(default = "default_mqtt_port")]
    pub broker_port: u16,
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
        let identifier_regex = IDENTIFIER_REGEX.get_or_init(|| Regex::new("[a-zA-Z0-9_]+").unwrap());

        let id = String::deserialize(de)?;

        if identifier_regex.is_match(&id) {
            Ok(Identifier(id))
        } else {
            Err(Error::custom("identifier must match [a-zA-Z0-9_]+"))
        }
    }
}

#[derive(Deserialize)]
pub struct CoverGroup {
    pub group_gpio_pause_ms: Option<u64>,
    pub devices: Vec<CoverConfig>,
}

#[derive(Deserialize)]
pub struct CoverConfig {
    pub name: String,
    pub chip: PathBuf,
    pub up_pin: u32,
    pub down_pin: u32,
    pub stop_pin: u32,
    pub device_gpio_pause_ms: Option<u64>,
    pub device: Device,
}

#[derive(Deserialize)]
pub struct SunspecConfig {
    pub name: String,
    pub device: Device,
    pub host: String,
    #[serde(default = "default_sunspec_port")]
    pub host_port: u16,
    pub device_polling_delay_ms: u64,
}
