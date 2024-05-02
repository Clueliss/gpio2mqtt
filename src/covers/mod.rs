pub mod stateless_gpio;

use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};
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

impl Display for CoverCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CoverCommand::Open => write!(f, "OPEN"),
            CoverCommand::Close => write!(f, "CLOSE"),
            CoverCommand::Stop => write!(f, "STOP"),
        }
    }
}
