use rumqttc::{AsyncClient, ClientError, QoS};
use serde::Serialize;

use crate::config;

const MQTT_BASE_TOPIC: &str = "gpio2mqtt";
const MQTT_DISCOVERY_TOPIC: &str = "homeassistant";
const MQTT_AVAIL_TOPIC: &str = "gpio2mqtt/bridge/state";

pub async fn register_covers(
    client: &AsyncClient,
    payloads: &[ConfigurationPayload],
) -> Result<(), ClientError> {
    for payload in payloads {
        client
            .publish(
                format!(
                    "{MQTT_DISCOVERY_TOPIC}/cover/{}/cover/config",
                    payload.unique_id
                ),
                QoS::AtLeastOnce,
                true,
                serde_json::to_vec(payload).unwrap(),
            )
            .await?;

        client
            .subscribe(&payload.command_topic, QoS::AtLeastOnce)
            .await?;
    }

    Ok(())
}

pub async fn announce_online(client: &AsyncClient) -> Result<(), ClientError> {
    client
        .publish(MQTT_AVAIL_TOPIC, QoS::AtLeastOnce, true, "online")
        .await
}

pub async fn announce_offline(client: &AsyncClient) -> Result<(), ClientError> {
    client
        .publish(MQTT_AVAIL_TOPIC, QoS::AtLeastOnce, true, "offline")
        .await
}

pub fn command_topic_for_dev_id(dev_id: &config::Identifier) -> String {
    format!("{MQTT_BASE_TOPIC}/{dev_id}/set", dev_id = dev_id.0)
}

#[derive(Serialize, Debug)]
pub struct ConfigurationPayload {
    name: String,
    unique_id: String,
    command_topic: String,
    availability: Vec<AvailabilityPayload>,
    device: DevicePayload,
}

#[derive(Serialize, Debug)]
pub struct AvailabilityPayload {
    topic: String,
}

#[derive(Serialize, Debug)]
pub struct DevicePayload {
    name: String,
    identifiers: Vec<String>,
    manufacturer: String,
    model: String,
}

impl From<config::CoverConfig> for ConfigurationPayload {
    fn from(conf: config::CoverConfig) -> Self {
        let dev_id = conf.device.identifier;

        Self {
            unique_id: format!("{MQTT_BASE_TOPIC}_{dev_id}", dev_id = dev_id.0),
            command_topic: command_topic_for_dev_id(&dev_id),
            availability: vec![AvailabilityPayload {
                topic: MQTT_AVAIL_TOPIC.to_string(),
            }],
            device: DevicePayload {
                name: conf.name.clone(),
                identifiers: vec![dev_id.0],
                manufacturer: conf.device.manufacturer,
                model: conf.device.model,
            },
            name: conf.name,
        }
    }
}
