mod config;
mod covers;
mod eventloop;
mod mqtt;
mod sunspec;

use anyhow::{Context, Result};
use paho_mqtt::{AsyncClient, ConnectOptionsBuilder, CreateOptionsBuilder, PersistenceType};
use std::{collections::HashMap, fs::File, net::SocketAddr, sync::Arc};
use tokio::{
    select,
    sync::{mpsc, Mutex},
    time::Duration,
};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let config_path = if cfg!(debug_assertions) {
        "./gpio2mqtt.yaml"
    } else {
        "/etc/gpio2mqtt.yaml"
    };

    let config = File::open(config_path).with_context(|| format!("Failed to open config file {config_path:?}"))?;

    let config: config::Config =
        serde_yaml::from_reader(config).with_context(|| format!("Failed to parse config file {config_path:?}"))?;

    let covers: Vec<(Duration, HashMap<_, _>)> = config
        .covers
        .iter()
        .flatten()
        .map(|cover_group| {
            let covers = cover_group
                .devices
                .iter()
                .map(|cover_conf| {
                    Ok((
                        mqtt::command_topic_for_dev_id(&config.client_id, &cover_conf.device.identifier),
                        (
                            Duration::from_millis(cover_conf.device_gpio_pause_ms.unwrap_or_default()),
                            covers::stateless_gpio::Cover::from_chip_offsets(
                                &cover_conf.chip,
                                cover_conf.up_pin,
                                cover_conf.down_pin,
                                cover_conf.stop_pin,
                            )?,
                        ),
                    ))
                })
                .collect::<Result<_>>()?;

            Ok((
                Duration::from_millis(cover_group.group_gpio_pause_ms.unwrap_or_default()),
                covers,
            ))
        })
        .collect::<Result<_>>()
        .context("Failed to set up GPIO pins")?;

    let mut sunspec_devices: HashMap<_, _> = config
        .sunspec
        .iter()
        .flatten()
        .map(|sunspec_conf| {
            Ok((
                mqtt::state_topic_for_dev_id(&config.client_id, &sunspec_conf.device.identifier),
                (
                    Duration::from_millis(sunspec_conf.device_polling_delay_ms),
                    sunspec::varta::ElementSunspecClient::new(SocketAddr::new(
                        sunspec_conf.host.parse()?,
                        sunspec_conf.host_port,
                    )),
                ),
            ))
        })
        .collect::<Result<_>>()
        .context("Failed to setup sunspec devices")?;

    let payloads = {
        let mut payloads = Vec::new();

        for cover_group in config.covers.into_iter().flatten() {
            for cover_conf in cover_group.devices {
                payloads.push(mqtt::ConfigPayload::from_cover_config(&config.client_id, cover_conf));
            }
        }

        for sunspec_conf in config.sunspec.into_iter().flatten() {
            let (_, device) = sunspec_devices
                .get_mut(&mqtt::state_topic_for_dev_id(
                    &config.client_id,
                    &sunspec_conf.device.identifier,
                ))
                .unwrap();

            let specs = device.specifications().await.ok();
            payloads.extend(mqtt::ConfigPayload::from_sunspec(
                &config.client_id,
                sunspec_conf,
                specs.as_ref(),
            ));
        }

        payloads
    };

    let mut mqtt_client = AsyncClient::new(
        CreateOptionsBuilder::new()
            .server_uri(format!(
                "tcp://{host}:{port}",
                host = config.broker,
                port = config.broker_port
            ))
            .client_id(&config.client_id)
            .persistence(PersistenceType::None)
            .finalize(),
    )
    .context("Failed to create MQTT client")?;

    let mqtt_stream = mqtt_client.get_stream(128);

    mqtt_client
        .connect(
            ConnectOptionsBuilder::new()
                .automatic_reconnect(Duration::from_secs(2u64.pow(3)), Duration::from_secs(2u64.pow(12)))
                .max_inflight(128)
                .will_message(mqtt::offline_message(&config.client_id))
                .finalize(),
        )
        .await
        .context("Failed to connect to MQTT broker")?;

    mqtt::announce_online(&config.client_id, &mqtt_client)
        .await
        .context("Failed to announce online status")?;

    mqtt::register_devices(&mqtt_client, &payloads)
        .await
        .context("Failed to register devices")?;

    let (tx, mut rx) = mpsc::channel(1);

    for (topic, (polling_delay, device)) in sunspec_devices {
        tokio::spawn(eventloop::sunspec_event_loop(topic, polling_delay, device, tx.clone()));
    }

    let cover_channels = {
        let mut cover_channels = HashMap::new();

        for (group_delay, group) in covers {
            let group_gpio_pause = Arc::new(Mutex::new(eventloop::Pause::new(group_delay)));

            for (topic, (device_gpio_pause, device)) in group {
                let (tx, fut) = eventloop::stateless_cover_event_loop(
                    topic.clone(),
                    group_gpio_pause.clone(),
                    device_gpio_pause,
                    device,
                );

                cover_channels.insert(topic, tx);
                tokio::spawn(fut);
            }
        }

        cover_channels
    };

    tokio::spawn(eventloop::mqtt_message_event_loop(mqtt_stream, tx));

    loop {
        select! {
            _ = tokio::signal::ctrl_c() => {
                let _ = mqtt::announce_offline(&config.client_id, &mqtt_client).await;
                break Ok(());
            },
            event = rx.recv() => match event.unwrap() {
                eventloop::Message::SunspecMeasurement(topic, measurement) => {
                    mqtt::publish_state(&mqtt_client, topic, &mqtt::SunspecState::from(measurement))
                        .await
                        .context("Unable to publish state")?;
                },
                eventloop::Message::MqttEvent(msg) => {
                    let payload = match std::str::from_utf8(msg.payload()) {
                        Ok(payload) => payload,
                        Err(e) => {
                            eprintln!("MQTT payload error: {e}");
                            continue;
                        }
                    };

                    println!("MQTT command incoming: topic '{}' payload '{}'", msg.topic(), payload);

                    let Some(chan) = cover_channels.get(msg.topic()) else {
                        eprintln!("MQTT command error: unknown cover at {}", msg.topic());
                        continue;
                    };

                    let cmd = match payload.parse() {
                        Ok(cmd) => cmd,
                        Err(e) => {
                            eprintln!("MQTT payload error: {e}");
                            continue;
                        },
                    };

                    chan.send(cmd).unwrap();
                },
            }
        }
    }
}
