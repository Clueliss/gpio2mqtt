mod registers;

use super::{Percentage, Quantity, WattHours, Watts};
use modbus::{Modbus, Register};
use serde::{Serialize, Serializer};
use std::net::SocketAddr;
use tokio::sync::OnceCell;

#[derive(Serialize, Debug, PartialEq, Eq)]
pub enum GridPower {
    Backfeed(Watts),
    Consumption(Watts),
}

impl GridPower {
    fn serialize<S: Serializer>(power: &Option<GridPower>, s: S) -> Result<S::Ok, S::Error> {
        match *power {
            None => s.serialize_i16(0),
            Some(GridPower::Backfeed(amount)) => s.serialize_i32(amount as i32),
            Some(GridPower::Consumption(amount)) => s.serialize_i32(-(amount as i32)),
        }
    }
}

#[derive(Serialize, Debug, PartialEq, Eq)]
pub enum BatteryPower {
    Charge(Watts),
    Discharge(Watts),
}

impl BatteryPower {
    fn serialize<S: Serializer>(power: &Option<BatteryPower>, s: S) -> Result<S::Ok, S::Error> {
        match *power {
            None => s.serialize_i16(0),
            Some(BatteryPower::Charge(amount)) => s.serialize_i32(amount as i32),
            Some(BatteryPower::Discharge(amount)) => s.serialize_i32(-(amount as i32)),
        }
    }
}

#[derive(Serialize, Debug, PartialEq, Eq)]
pub enum State {
    Busy,
    Ready,
    Charge,
    Discharge,
    Standby,
    Error,
    Passive,
    IsLanding,
}

impl TryFrom<u16> for State {
    type Error = ();

    fn try_from(value: u16) -> Result<Self, <Self as TryFrom<u16>>::Error> {
        use State::*;

        match value {
            0 => Ok(Busy),
            1 => Ok(Ready),
            2 => Ok(Charge),
            3 => Ok(Discharge),
            4 => Ok(Standby),
            5 => Ok(Error),
            6 => Ok(Passive),
            7 => Ok(IsLanding),
            _ => Err(()),
        }
    }
}

#[derive(Serialize, Debug)]
pub struct Measurements {
    pub state: State,
    pub state_of_charge: Percentage,
    pub total_charge_energy: WattHours,

    #[serde(serialize_with = "BatteryPower::serialize")]
    pub active_power: Option<BatteryPower>,
    #[serde(serialize_with = "BatteryPower::serialize")]
    pub apparent_power: Option<BatteryPower>,
    #[serde(serialize_with = "GridPower::serialize")]
    pub grid_power: Option<GridPower>,
}

#[derive(Serialize, Debug, Clone)]
pub struct DeviceSpecifications {
    pub software_version_ems: [u16; 17],
    pub software_version_ens: [u16; 17],
    pub software_version_inverter: [u16; 17],
    pub table_version: u16,
    pub serial_number: [u16; 10],
    pub installed_battery_modules: Quantity,
    pub installed_battery_capacity: WattHours,
}

pub struct ElementSunspecClient {
    client: Modbus,
    spec_cache: OnceCell<DeviceSpecifications>,
}

impl ElementSunspecClient {
    pub fn new(addr: SocketAddr) -> Self {
        Self { client: Modbus::new(addr), spec_cache: Default::default() }
    }

    pub async fn specifications(&mut self) -> modbus::Result<DeviceSpecifications> {
        self.spec_cache
            .get_or_try_init(|| async {
                let response1 = self
                    .client
                    .read_input_registers(
                        registers::SOFTWARE_VERSION_EMS.start..registers::INSTALLED_BATTERY_MODULES.end,
                    )
                    .await?;

                let response2 = self
                    .client
                    .read_input_registers(registers::INSTALLED_BATTERY_CAPACITY)
                    .await?;

                let slice = |reg: Register| {
                    if reg == registers::INSTALLED_BATTERY_CAPACITY {
                        &response2[..]
                    } else {
                        &response1[(reg.start - registers::SOFTWARE_VERSION_EMS.start) as usize
                            ..(reg.end - registers::SOFTWARE_VERSION_EMS.start) as usize]
                    }
                };

                let specs = DeviceSpecifications {
                    software_version_ems: slice(registers::SOFTWARE_VERSION_EMS).try_into().unwrap(),
                    software_version_ens: slice(registers::SOFTWARE_VERSION_ENS).try_into().unwrap(),
                    software_version_inverter: slice(registers::SOFTWARE_VERSION_INVERTER).try_into().unwrap(),
                    table_version: slice(registers::TABLE_VERSION)[0],
                    serial_number: slice(registers::SERIAL_NUMBER).try_into().unwrap(),
                    installed_battery_modules: slice(registers::INSTALLED_BATTERY_MODULES)[0],
                    installed_battery_capacity: slice(registers::INSTALLED_BATTERY_CAPACITY)[0] as u32,
                };

                Ok(specs)
            })
            .await
            .cloned()
    }

    pub async fn measure(&mut self) -> modbus::Result<Measurements> {
        let response1 = self
            .client
            .read_input_registers(registers::STATE.start..registers::TOTAL_CHARGE_ENERGY.end)
            .await?;

        let response2 = self.client.read_input_registers(registers::GRID_POWER).await?;

        let slice = |reg: Register| {
            if reg == registers::GRID_POWER {
                &response2[..]
            } else {
                &response1[(reg.start - registers::STATE.start) as usize..(reg.end - registers::STATE.start) as usize]
            }
        };

        Ok(Measurements {
            state: slice(registers::STATE)[0].try_into().unwrap(),
            active_power: {
                let value = slice(registers::ACTIVE_POWER)[0] as i16;
                match value {
                    ..=-1 => Some(BatteryPower::Discharge(value.unsigned_abs())),
                    0 => None,
                    1.. => Some(BatteryPower::Charge(value as u16)),
                }
            },
            apparent_power: {
                let value = slice(registers::APPARENT_POWER)[0] as i16;
                match value {
                    ..=-1 => Some(BatteryPower::Discharge(value.unsigned_abs())),
                    0 => None,
                    1.. => Some(BatteryPower::Charge(value as u16)),
                }
            },
            state_of_charge: slice(registers::STATE_OF_CHARGE)[0],
            total_charge_energy: {
                let slice = slice(registers::TOTAL_CHARGE_ENERGY);
                let lower_word = slice[0];
                let upper_word = slice[1];

                lower_word as u32 | ((upper_word as u32) << 16)
            },
            grid_power: {
                let value = slice(registers::GRID_POWER)[0] as i16;
                match value {
                    ..=-1 => Some(GridPower::Consumption(value.unsigned_abs())),
                    0 => None,
                    1.. => Some(GridPower::Backfeed(value as u16)),
                }
            },
        })
    }
}