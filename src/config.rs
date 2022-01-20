use std::path::PathBuf;

use lazy_static::lazy_static;
use serde::{de::Error, Deserialize, Deserializer, Serialize};

lazy_static! {
    static ref IDENTIFIER_REGEX: regex::Regex = regex::Regex::new("[a-zA-Z0-9_]+").unwrap();
}

const fn default_port() -> u16 {
    1883
}

#[derive(Deserialize)]
pub struct Config {
    pub covers: Vec<CoverConfig>,
    pub host: String,

    #[serde(default = "default_port")]
    pub port: u16,
}

#[derive(Deserialize)]
pub struct CoverConfig {
    pub name: String,
    pub chip: PathBuf,
    pub up_pin: u32,
    pub down_pin: u32,
    pub stop_pin: u32,
    pub device: Device,
}

#[derive(Serialize, Debug)]
pub struct Identifier(pub String);

#[derive(Serialize, Deserialize, Debug)]
pub struct Device {
    pub identifier: Identifier,
    pub manufacturer: String,
    pub model: String,
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
            Err(D::Error::custom("identifier must match [a-zA-Z0-9_]+"))
        }
    }
}
