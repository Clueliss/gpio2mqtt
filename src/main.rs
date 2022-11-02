#![feature(pin_macro)]

mod config;
mod covers;
mod mqtt;
mod sunspec;

use std::pin::pin;
use anyhow::Result;
use covers::CoverCommand;
use rumqttc::{AsyncClient, ConnectionError, Event, Incoming, MqttOptions, Publish};
use std::{collections::HashMap, fs::File, net::SocketAddr, sync::Arc};
use tokio::{
    select,
    sync::Mutex,
    time::{Duration, Instant, MissedTickBehavior},
    sync::mpsc,
};

enum Message {
    Tick,
    MqttEvent(Result<Event, ConnectionError>),
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let config: config::Config = serde_yaml::from_reader(File::open("/etc/gpio2mqtt.yaml")?)?;

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
                Duration::from_millis(cover_conf.device.tx_timeout_ms.unwrap_or_default()),
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

    let opts = MqttOptions::new("gpio2mqtt_bridge", config.host, config.port);
    let (client, mut eventloop) = AsyncClient::new(opts, 10);

    let mut sensor_timer = tokio::time::interval(Duration::from_secs(10));
    sensor_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let transmission_timeout = Arc::new(Mutex::new(Instant::now()));


    let (tx, mut rx) = mpsc::channel(128);

    {
        let tx = tx.clone();
        tokio::spawn(async move {
            loop {
                sensor_timer.tick().await;
                if let Err(_) = tx.send(Message::Tick).await {
                    return;
                }
            }
        });
    }

    {
        let tx = tx.clone();
        tokio::spawn(async move {
            loop {
                let event = eventloop.poll().await;
                if let Err(_) = tx.send(Message::MqttEvent(event)).await {
                    break;
                }
            }
        });
    }

    loop {
        select! {
            _ = tokio::signal::ctrl_c() => {
                let _ = mqtt::announce_offline(&client).await;
                break;
            },
            event = rx.recv() => match event.unwrap() {
                Message::Tick => {
                    for (topic, sunspec) in &mut sunspec_devices {
                        match sunspec.measure().await {
                            Ok(measurements) => mqtt::publish_state(&client, topic, &measurements).await?,
                            Err(e) => eprintln!("Error unable to read from sunspec modbus: {e}"),
                        }
                    }
                },
                Message::MqttEvent(Ok(Event::Incoming(Incoming::ConnAck(_)))) => {
                    println!("MQTT connection ack incoming: announcing capabilities");
                    mqtt::announce_online(&client).await?;
                    mqtt::register_devices(&client, &payloads).await?;
                },
                Message::MqttEvent(Ok(Event::Incoming(Incoming::Publish(Publish { topic, payload, .. })))) => {
                    println!("MQTT command incoming: topic '{topic}' payload '{payload:?}'");

                    let Some(cover) = covers.get(&topic) else {
                        eprintln!("MQTT error: unknown cover at {topic}");
                        continue;
                    };

                    let payload = match String::from_utf8(payload.to_vec()) {
                        Ok(payload) => payload,
                        Err(e) => {
                            eprintln!("MQTT error: {e}");
                            continue;
                        }
                    };

                    let cmd = match payload.parse() {
                        Ok(cmd) => cmd,
                        Err(e) => {
                            eprintln!("MQTT error: {e}");
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
                Message::MqttEvent(Err(ConnectionError::Io(e))) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                    eprintln!("MQTT io error: {e}, retrying in 10s");
                    tokio::time::sleep(Duration::from_secs(10)).await;
                },
                _ => (),
            }
        }
    }

    Ok(())
}
