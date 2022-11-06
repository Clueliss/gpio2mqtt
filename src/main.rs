mod config;
mod covers;
mod mqtt;
mod sunspec;

use anyhow::Result;
use covers::CoverCommand;
use paho_mqtt::{AsyncClient, ConnectOptionsBuilder, CreateOptionsBuilder};
use std::{collections::HashMap, fs::File, net::SocketAddr, sync::Arc};
use tokio::{
    select,
    sync::{mpsc, Mutex},
    time::{Duration, Instant, MissedTickBehavior},
};

enum Message {
    Tick,
    MqttEvent(Option<paho_mqtt::Message>),
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let config: config::Config = serde_yaml::from_reader(File::open(if cfg!(debug_assertions) {
        "./gpio2mqtt.yaml"
    } else {
        "/etc/gpio2mqtt.yaml"
    })?)?;

    let covers: HashMap<_, _> = config
        .covers
        .iter()
        .flatten()
        .map(|cover_conf| {
            let opts = covers::stateless_gpio::Options::from_chip_offsets(
                &cover_conf.chip,
                cover_conf.up_pin,
                cover_conf.down_pin,
                cover_conf.stop_pin,
                Duration::from_millis(cover_conf.tx_timeout_ms.unwrap_or_default()),
                &cover_conf.device.identifier,
            )?;

            Ok((
                mqtt::command_topic_for_dev_id(&cover_conf.device.identifier),
                Arc::new(covers::stateless_gpio::Cover::new(opts)),
            ))
        })
        .collect::<Result<_>>()?;

    let mut sunspec_devices = {
        let mut tmp: HashMap<_, _> = Default::default();

        for sunspec_conf in config.sunspec_devices.iter().flatten() {
            tmp.insert(
                mqtt::state_topic_for_dev_id(&sunspec_conf.device.identifier),
                sunspec::varta::ElementSunspecClient::new(SocketAddr::new(
                    sunspec_conf.host.parse()?,
                    sunspec_conf.port,
                )),
            );
        }

        tmp
    };

    let payloads: Vec<mqtt::MqttConfigPayload> = config
        .covers
        .into_iter()
        .flatten()
        .map(Into::into)
        .chain(config.sunspec_devices.into_iter().flatten().flat_map(Vec::<_>::from))
        .collect();

    let mut mqtt_client = AsyncClient::new(
        CreateOptionsBuilder::new()
            .server_uri(format!("tcp://{host}:{port}", host = config.host, port = config.port))
            .client_id(if cfg!(debug_assertions) {
                "gpio2mqtt_bridge2"
            } else {
                "gpio2mqtt_bridge"
            })
            .finalize(),
    )?;

    let mqtt_stream = mqtt_client.get_stream(128);

    mqtt_client
        .connect(
            ConnectOptionsBuilder::new()
                .automatic_reconnect(Duration::from_secs(2u64.pow(3)), Duration::from_secs(2u64.pow(12)))
                .max_inflight(128)
                .will_message(mqtt::offline_message())
                .finalize(),
        )
        .await?;

    mqtt::announce_online(&mqtt_client).await?;
    mqtt::register_devices(&mqtt_client, &payloads).await?;

    let transmission_timeout = Arc::new(Mutex::new(Instant::now()));

    let (tx, mut rx) = mpsc::channel(1);

    {
        let mut sensor_timer = tokio::time::interval(Duration::from_secs(1));
        sensor_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

        let tx = tx.clone();
        tokio::spawn(async move {
            loop {
                sensor_timer.tick().await;
                if let Err(_) = tx.send(Message::Tick).await {
                    println!("Shutting down update timer");
                    return;
                }
            }
        });
    }

    {
        let tx = tx;
        tokio::spawn(async move {
            loop {
                let Ok(event) = mqtt_stream.recv().await else {
                    break;
                };

                let Ok(_) = tx.send(Message::MqttEvent(event)).await else {
                    println!("Shutting down MQTT client");
                    break;
                };
            }
        });
    }

    loop {
        select! {
            _ = tokio::signal::ctrl_c() => {
                let _ = mqtt::announce_offline(&mqtt_client).await;
                break Ok(());
            },
            event = rx.recv() => match event.unwrap() {
                Message::Tick => {
                    for (topic, sunspec) in &mut sunspec_devices {
                        match sunspec.measure().await {
                            Ok(measurements) => mqtt::publish_state(&mqtt_client, topic, &mqtt::SunspecState::from(measurements)).await?,
                            Err(e) => eprintln!("Error unable to read from sunspec modbus: {e}"),
                        }
                    }
                },
                Message::MqttEvent(Some(msg)) => {
                    let payload = match std::str::from_utf8(msg.payload()) {
                        Ok(payload) => payload,
                        Err(e) => {
                            eprintln!("MQTT payload error: {e}");
                            continue;
                        }
                    };

                    println!("MQTT command incoming: topic '{}' payload '{}'", msg.topic(), payload);

                    let Some(cover) = covers.get(msg.topic()) else {
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

                    let c = cover.clone();
                    let tt = transmission_timeout.clone();

                    tokio::spawn(async move {
                        let mut tt = tt.lock().await;
                        tokio::time::sleep_until(*tt).await;

                        let t1 = Instant::now();

                        match cmd {
                            CoverCommand::Open => c.move_up().await?,
                            CoverCommand::Close => c.move_down().await?,
                            CoverCommand::Stop => c.stop().await?,
                        }

                        let elapsed = Instant::now() - t1;
                        *tt = Instant::now() + (Duration::from_millis(config.global_tx_timeout_ms) - elapsed);

                        Ok::<_, anyhow::Error>(())
                    });
                },
                Message::MqttEvent(None) => {
                    break Err(anyhow::Error::msg("Lost connection to server"));
                },
            }
        }
    }
}
