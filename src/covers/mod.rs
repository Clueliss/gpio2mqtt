pub mod stateless_gpio;

use std::str::FromStr;
use thiserror::Error;

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum CoverCommand {
    Open,
    Close,
    Stop,
}

#[derive(Error, Debug)]
#[error("invalid cover command")]
pub struct CoverCommandParseError;

impl FromStr for CoverCommand {
    type Err = CoverCommandParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "OPEN" => Ok(CoverCommand::Open),
            "CLOSE" => Ok(CoverCommand::Close),
            "STOP" => Ok(CoverCommand::Stop),
            _ => Err(CoverCommandParseError),
        }
    }
}

impl ToString for CoverCommand {
    fn to_string(&self) -> String {
        match self {
            CoverCommand::Open => "OPEN".to_owned(),
            CoverCommand::Close => "CLOSE".to_owned(),
            CoverCommand::Stop => "STOP".to_owned(),
        }
    }
}
