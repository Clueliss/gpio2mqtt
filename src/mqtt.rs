use paho_mqtt::{AsyncClient, Message};
use serde::Serialize;

use crate::{
    config,
    sunspec::{
        varta::{BatteryPower, GridPower, Measurements, State},
        Percentage, WattHours, Watts,
    },
};

const MQTT_BASE_TOPIC: &str = "gpio2mqtt";
const MQTT_DISCOVERY_TOPIC: &str = "homeassistant";
const MQTT_AVAIL_TOPIC: &str = "gpio2mqtt/bridge/state";

const QOS_AT_MOST_ONCE: i32 = paho_mqtt::QOS_0;
const QOS_AT_LEAST_ONCE: i32 = paho_mqtt::QOS_1;
const QOS_EXACTLY_ONCE: i32 = paho_mqtt::QOS_2;

pub async fn register_devices(client: &AsyncClient, payloads: &[MqttConfigPayload]) -> anyhow::Result<()> {
    for payload in payloads {
        println!(
            "MQTT publish: topic '{}' payload '{}'",
            payload.config_topic,
            serde_json::to_string(payload).unwrap()
        );

        let c = client.clone();
        let p = payload.clone();

        c.publish(Message::new_retained(
            &p.config_topic,
            serde_json::to_vec(&p).unwrap(),
            QOS_AT_LEAST_ONCE,
        ))
        .await?;

        if let DeviceSpecificMqttConfig::Cover { command_topic, .. } = &payload.specific {
            client.subscribe(command_topic, QOS_AT_LEAST_ONCE).await?;
        }
    }

    Ok(())
}

pub async fn announce_online(client: &AsyncClient) -> anyhow::Result<()> {
    client
        .publish(Message::new_retained(
            MQTT_AVAIL_TOPIC,
            b"online".to_owned(),
            QOS_AT_LEAST_ONCE,
        ))
        .await?;
    Ok(())
}

pub fn offline_message() -> Message {
    Message::new_retained(MQTT_AVAIL_TOPIC, "offline".to_owned(), QOS_AT_LEAST_ONCE)
}

pub async fn announce_offline(client: &AsyncClient) -> anyhow::Result<()> {
    client.publish(offline_message()).await?;
    Ok(())
}

pub async fn publish_state(client: &AsyncClient, topic: &str, payload: &impl Serialize) -> anyhow::Result<()> {
    println!(
        "MQTT publish topic: '{}' payload: '{}'",
        topic,
        serde_json::to_string(payload).unwrap()
    );

    client
        .publish(Message::new(
            topic,
            serde_json::to_vec(payload).unwrap(),
            QOS_AT_LEAST_ONCE,
        ))
        .await?;

    Ok(())
}

pub fn command_topic_for_dev_id(dev_id: &config::Identifier) -> String {
    format!("{MQTT_BASE_TOPIC}/{dev_id}/set", dev_id = dev_id.0)
}

pub fn state_topic_for_dev_id(dev_id: &config::Identifier) -> String {
    format!("{MQTT_BASE_TOPIC}/{dev_id}/state", dev_id = dev_id.0)
}

#[derive(Serialize, Debug, Clone)]
pub struct AvailabilityPayload {
    topic: String,
}

#[derive(Serialize, Debug, Default, Clone)]
pub struct DevicePayload {
    name: String,
    identifiers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    manufacturer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sw_version: Option<String>,
}

#[derive(Serialize, Debug, Copy, Clone)]
#[serde(rename_all = "snake_case")]
pub enum StateClass {
    Measurement,
    Total,
    TotalIncreasing,
}

#[derive(Serialize, Debug, Copy, Clone)]
#[serde(rename_all = "snake_case")]
pub enum DeviceClass {
    ApparentPower,
    Aqi,
    Battery,
    CarbonDioxide,
    CarbonMonoxide,
    Current,
    Date,
    Distance,
    Duration,
    Energy,
    Frequency,
    Gas,
    Humidity,
    Illuminance,
    Moisture,
    Monetary,
    NitrogenOxide,
    NitrogenMonoxide,
    NitrousOxide,
    Ozone,
    Pm1,
    Pm10,
    Pm25,
    PowerFactor,
    Power,
    Pressure,
    ReactivePower,
    SignalStrength,
    Speed,
    SulphurDioxide,
    Temperature,
    Timestamp,
    VolatileOrganicCompounds,
    Voltage,
    Volume,
    Weight,
}

#[derive(Serialize, Debug)]
pub struct SunspecState {
    state: State,
    state_of_charge: Percentage,
    total_charge_energy: WattHours,
    battery_active_charge_power: Watts,
    battery_active_discharge_power: Watts,
    grid_backfeed_power: Watts,
    grid_consumption_power: Watts,
}

impl From<Measurements> for SunspecState {
    fn from(value: Measurements) -> Self {
        Self {
            state: value.state,
            state_of_charge: value.state_of_charge,
            total_charge_energy: value.total_charge_energy,
            battery_active_charge_power: match value.active_battery_power {
                Some(BatteryPower::Charge(w)) => w,
                _ => 0,
            },
            battery_active_discharge_power: match value.active_battery_power {
                Some(BatteryPower::Discharge(w)) => w,
                _ => 0,
            },
            grid_backfeed_power: match value.grid_power {
                Some(GridPower::Backfeed(w)) => w,
                _ => 0,
            },
            grid_consumption_power: match value.grid_power {
                Some(GridPower::Consumption(w)) => w,
                _ => 0,
            },
        }
    }
}

#[derive(Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum DeviceSpecificMqttConfig {
    Cover {
        command_topic: String,
    },
    Sensor {
        state_topic: String,
        state_class: StateClass,

        #[serde(skip_serializing_if = "Option::is_none")]
        unit_of_measurement: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        value_template: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        device_class: Option<DeviceClass>,
    },
}

#[derive(Serialize, Debug, Clone)]
pub struct MqttConfigPayload {
    pub name: String,
    pub unique_id: String,
    pub availability: Vec<AvailabilityPayload>,
    pub device: DevicePayload,
    pub config_topic: String,

    #[serde(flatten)]
    pub specific: DeviceSpecificMqttConfig,
}

impl From<config::CoverConfig> for MqttConfigPayload {
    fn from(conf: config::CoverConfig) -> Self {
        let dev_id = conf.device.identifier;
        let unique_id = format!("{MQTT_BASE_TOPIC}_{dev_id}", dev_id = dev_id.0);

        Self {
            config_topic: format!("{MQTT_DISCOVERY_TOPIC}/cover/{unique_id}/config"),
            unique_id,
            specific: DeviceSpecificMqttConfig::Cover { command_topic: command_topic_for_dev_id(&dev_id) },
            availability: vec![AvailabilityPayload { topic: MQTT_AVAIL_TOPIC.to_string() }],
            device: DevicePayload {
                name: conf.name.clone(),
                identifiers: vec![dev_id.0],
                manufacturer: conf.device.manufacturer,
                model: conf.device.model,
                sw_version: None,
            },
            name: conf.name,
        }
    }
}

impl From<config::SunspecConfig> for Vec<MqttConfigPayload> {
    fn from(conf: config::SunspecConfig) -> Self {
        let dev_id = conf.device.identifier;

        let state_topic = state_topic_for_dev_id(&dev_id);

        let sensors = vec![
            (
                "state",
                DeviceSpecificMqttConfig::Sensor {
                    state_topic: state_topic.clone(),
                    device_class: None,
                    state_class: StateClass::Measurement,
                    unit_of_measurement: None,
                    value_template: Some("{{ value_json.state }}".to_owned()),
                },
            ),
            (
                "battery_active_charge_power",
                DeviceSpecificMqttConfig::Sensor {
                    state_topic: state_topic.clone(),
                    device_class: Some(DeviceClass::Power),
                    state_class: StateClass::Measurement,
                    unit_of_measurement: Some("W".to_owned()),
                    value_template: Some("{{ value_json.battery_active_charge_power }}".to_owned()),
                },
            ),
            (
                "battery_active_discharge_power",
                DeviceSpecificMqttConfig::Sensor {
                    state_topic: state_topic.clone(),
                    device_class: Some(DeviceClass::Power),
                    state_class: StateClass::Measurement,
                    unit_of_measurement: Some("W".to_owned()),
                    value_template: Some("{{ value_json.battery_active_discharge_power }}".to_owned()),
                },
            ),
            (
                "state_of_charge",
                DeviceSpecificMqttConfig::Sensor {
                    state_topic: state_topic.clone(),
                    device_class: Some(DeviceClass::Battery),
                    state_class: StateClass::Measurement,
                    unit_of_measurement: Some("%".to_owned()),
                    value_template: Some("{{ value_json.state_of_charge }}".to_owned()),
                },
            ),
            (
                "total_charge_energy",
                DeviceSpecificMqttConfig::Sensor {
                    state_topic: state_topic.clone(),
                    device_class: Some(DeviceClass::Energy),
                    state_class: StateClass::TotalIncreasing,
                    unit_of_measurement: Some("Wh".to_owned()),
                    value_template: Some("{{ value_json.total_charge_energy }}".to_owned()),
                },
            ),
            (
                "grid_consumption_power",
                DeviceSpecificMqttConfig::Sensor {
                    state_topic: state_topic.clone(),
                    device_class: Some(DeviceClass::Power),
                    state_class: StateClass::Measurement,
                    unit_of_measurement: Some("W".to_owned()),
                    value_template: Some("{{ value_json.grid_consumption_power }}".to_owned()),
                },
            ),
            (
                "grid_backfeed_power",
                DeviceSpecificMqttConfig::Sensor {
                    state_topic: state_topic.clone(),
                    device_class: Some(DeviceClass::Power),
                    state_class: StateClass::Measurement,
                    unit_of_measurement: Some("W".to_owned()),
                    value_template: Some("{{ value_json.grid_backfeed_power }}".to_owned()),
                },
            ),
        ];

        let unique_id = format!("{MQTT_BASE_TOPIC}_{dev_id}", dev_id = dev_id.0);

        sensors
            .into_iter()
            .map(move |(name, sensor)| MqttConfigPayload {
                config_topic: format!("{MQTT_DISCOVERY_TOPIC}/sensor/{unique_id}/{name}/config"),
                unique_id: format!("{unique_id}_{name}"),
                availability: vec![AvailabilityPayload { topic: MQTT_AVAIL_TOPIC.to_string() }],
                device: DevicePayload {
                    name: conf.name.clone(),
                    identifiers: vec![dev_id.0.clone()],
                    manufacturer: conf.device.manufacturer.clone(),
                    model: conf.device.model.clone(),
                    sw_version: conf.device.sw_version.clone(),
                },
                name: format!("{} {name}", conf.name),
                specific: sensor,
            })
            .collect()
    }
}
