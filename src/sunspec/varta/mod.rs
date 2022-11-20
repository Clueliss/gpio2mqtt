mod registers;

use super::{Percentage, Quantity, WattHours, Watts};
use crate::sunspec::VoltAmps;
use modbus::{Modbus, Register};
use serde::Serialize;
use std::net::SocketAddr;

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum GridPower {
    Backfeed(Watts),
    Consumption(Watts),
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum BatteryPower<T> {
    Charge(T),
    Discharge(T),
}

pub type ActiveBatteryPower = BatteryPower<Watts>;
pub type ApparentBatteryPower = BatteryPower<VoltAmps>;

#[derive(Serialize, Debug, PartialEq, Eq, Copy, Clone)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Busy,
    Ready,
    Charging,
    Discharging,
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
            2 => Ok(Charging),
            3 => Ok(Discharging),
            4 => Ok(Standby),
            5 => Ok(Error),
            6 => Ok(Passive),
            7 => Ok(IsLanding),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Measurements {
    pub state: State,
    pub state_of_charge: Percentage,
    pub total_charge_energy: WattHours,
    pub active_battery_power: Option<ActiveBatteryPower>,
    pub apparent_battery_power: Option<ApparentBatteryPower>,
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
}

impl ElementSunspecClient {
    pub fn new(addr: SocketAddr) -> Self {
        Self { client: Modbus::new(addr) }
    }

    pub async fn specifications(&mut self) -> modbus::Result<DeviceSpecifications> {
        let response1 = self
            .client
            .read_input_registers(registers::SOFTWARE_VERSION_EMS.start..registers::INSTALLED_BATTERY_MODULES.end)
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

        Ok(DeviceSpecifications {
            software_version_ems: slice(registers::SOFTWARE_VERSION_EMS).try_into().unwrap(),
            software_version_ens: slice(registers::SOFTWARE_VERSION_ENS).try_into().unwrap(),
            software_version_inverter: slice(registers::SOFTWARE_VERSION_INVERTER).try_into().unwrap(),
            table_version: slice(registers::TABLE_VERSION)[0],
            serial_number: slice(registers::SERIAL_NUMBER).try_into().unwrap(),
            installed_battery_modules: slice(registers::INSTALLED_BATTERY_MODULES)[0],
            installed_battery_capacity: slice(registers::INSTALLED_BATTERY_CAPACITY)[0] as u32,
        })
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
            active_battery_power: {
                let value = slice(registers::ACTIVE_POWER)[0] as i16;
                match value {
                    ..=-1 => Some(BatteryPower::Discharge(value.unsigned_abs())),
                    0 => None,
                    1.. => Some(BatteryPower::Charge(value as u16)),
                }
            },
            apparent_battery_power: {
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
