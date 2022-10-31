use rumqttc::{AsyncClient, ClientError, QoS};
use serde::Serialize;

use crate::config;

const MQTT_BASE_TOPIC: &str = "gpio2mqtt";
const MQTT_DISCOVERY_TOPIC: &str = "homeassistant";
const MQTT_AVAIL_TOPIC: &str = "gpio2mqtt/bridge/state";

pub async fn register_devices(client: &AsyncClient, payloads: &[MqttConfigPayload]) -> Result<(), ClientError> {
    for payload in payloads {
        client
            .publish(
                payload.config_topic.clone(),
                QoS::AtLeastOnce,
                true,
                serde_json::to_vec(payload).unwrap(),
            )
            .await?;

        if let DeviceSpecificMqttConfig::Cover { command_topic, .. } = &payload.specific {
            client.subscribe(command_topic, QoS::AtLeastOnce).await?;
        }
    }

    Ok(())
}

pub async fn announce_online(client: &AsyncClient) -> Result<(), ClientError> {
    client.publish(MQTT_AVAIL_TOPIC, QoS::AtLeastOnce, true, "online").await
}

pub async fn announce_offline(client: &AsyncClient) -> Result<(), ClientError> {
    client
        .publish(MQTT_AVAIL_TOPIC, QoS::AtLeastOnce, true, "offline")
        .await
}

pub async fn publish_state(client: &AsyncClient, topic: &str, payload: &impl Serialize) -> Result<(), ClientError> {
    client
        .publish(topic, QoS::AtLeastOnce, true, serde_json::to_vec(payload).unwrap())
        .await
}

pub fn command_topic_for_dev_id(dev_id: &config::Identifier) -> String {
    format!("{MQTT_BASE_TOPIC}/{dev_id}/set", dev_id = dev_id.0)
}

pub fn state_topic_for_dev_id(dev_id: &config::Identifier) -> String {
    format!("{MQTT_BASE_TOPIC}/{dev_id}/state", dev_id = dev_id.0)
}

pub fn subcomponent_state_topic(dev_id: &config::Identifier, subcomponent: &str) -> String {
    format!("{MQTT_BASE_TOPIC}/{dev_id}/{subcomponent}/state", dev_id = dev_id.0)
}

#[derive(Serialize, Debug)]
pub struct AvailabilityPayload {
    topic: String,
}

#[derive(Serialize, Debug, Default)]
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

#[derive(Serialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum StateClass {
    Measurement,
    Total,
    TotalIncreasing,
}

#[derive(Serialize, Debug)]
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

#[derive(Serialize, Debug)]
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

        let sensors = [
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
                "active_power",
                DeviceSpecificMqttConfig::Sensor {
                    state_topic: state_topic.clone(),
                    device_class: Some(DeviceClass::Power),
                    state_class: StateClass::Measurement,
                    unit_of_measurement: Some("W".to_owned()),
                    value_template: Some("{{ value_json.active_power }}".to_owned()),
                },
            ),
            (
                "apparent_power",
                DeviceSpecificMqttConfig::Sensor {
                    state_topic: state_topic.clone(),
                    device_class: Some(DeviceClass::ApparentPower),
                    state_class: StateClass::Measurement,
                    unit_of_measurement: Some("VA".to_owned()),
                    value_template: Some("{{ value_json.apparent_power }}".to_owned()),
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
                "grid_power",
                DeviceSpecificMqttConfig::Sensor {
                    state_topic,
                    device_class: Some(DeviceClass::Power),
                    state_class: StateClass::Measurement,
                    unit_of_measurement: Some("W".to_owned()),
                    value_template: Some("{{ value_json.grid_power }}".to_owned()),
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
