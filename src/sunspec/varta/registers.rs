use modbus::{Address, Register};
use std::ops::Range;

pub const REGISTER_BASE_ADDRESS: Address = 1000;
pub const DYNAMIC_REGISTER_RANGE1: Range<Address> = 1065..1071;
pub const DYNAMIC_REGISTER_RANGE2: Range<Address> = 1078..1079;

pub const SOFTWARE_VERSION_EMS: Register = 1000..1017;
pub const SOFTWARE_VERSION_ENS: Register = 1017..1034;
pub const SOFTWARE_VERSION_INVERTER: Register = 1034..1051;
pub const TABLE_VERSION: Register = 1051..1052;
pub const TIMESTAMP: Register = 1052..1054;
pub const SERIAL_NUMBER: Register = 1054..1064;
pub const INSTALLED_BATTERY_MODULES: Register = 1064..1065;
pub const STATE: Register = 1065..1066;
pub const ACTIVE_POWER: Register = 1066..1067;
pub const APPARENT_POWER: Register = 1067..1068;
pub const STATE_OF_CHARGE: Register = 1068..1069;
pub const TOTAL_CHARGE_ENERGY: Register = 1069..1071;
pub const INSTALLED_BATTERY_CAPACITY: Register = 1071..1072;
pub const GRID_POWER: Register = 1078..1079;

pub fn is_register_in_address_range(range: Range<Address>, reg: Register) -> bool {
    reg.start >= range.start && reg.end <= range.end
}
