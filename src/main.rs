mod config;
mod covers;
mod mqtt;
mod sunspec;

use std::{
    collections::HashMap,
    fs::File,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use covers::CoverCommand;
use rumqttc::{AsyncClient, ConnectionError, Event, Incoming, MqttOptions, Publish};
use tokio::{select, sync::Mutex};
use tokio::time::MissedTickBehavior;


#[derive(Debug)]
enum Message {
    MqttEvent(Result<Event, ConnectionError>),
    TickEvent,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let config: config::Config = serde_yaml::from_reader(File::open("/etc/gpio2mqtt.yaml").unwrap()).unwrap();

    let covers: HashMap<_, _> = config
        .covers
        .iter()
        .map(|cover_conf| {
            let opts = covers::stateless_gpio::Options::from_chip_offsets(
                &cover_conf.chip,
                cover_conf.up_pin,
                cover_conf.down_pin,
                cover_conf.stop_pin,
                Duration::from_millis(cover_conf.device.tx_timeout_ms.unwrap_or_default()),
                &cover_conf.device.identifier,
            )
            .unwrap();

            (
                mqtt::command_topic_for_dev_id(&cover_conf.device.identifier),
                Arc::new(covers::stateless_gpio::Cover::new(opts)),
            )
        })
        .collect();

    let mut sunspec_devices = {
        let mut tmp: HashMap<_, _> = Default::default();

        for sunspec_conf in &config.sunspec_devices {
            tmp.insert(
                mqtt::state_topic_for_dev_id(&sunspec_conf.device.identifier),
                sunspec::varta::ElementSunspecClient::new(SocketAddr::new(
                    sunspec_conf.host.parse().unwrap(),
                    sunspec_conf.port,
                )),
            );
        }

        tmp
    };

    let payloads: Vec<mqtt::MqttConfigPayload> = config
        .covers
        .into_iter()
        .map(Into::into)
        .chain(config.sunspec_devices.into_iter().flat_map(Vec::<_>::from))
        .collect();

    let opts = MqttOptions::new("gpio2mqtt_bridge", config.host, config.port);
    let (client, mut eventloop) = AsyncClient::new(opts, 10);

    let mut sensor_timer = tokio::time::interval(Duration::from_secs(10));
    sensor_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let transmission_timeout = Arc::new(Mutex::new(Instant::now()));

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    let tx1 = tx.clone();
    tokio::spawn(async move {
        loop {
            let event = eventloop.poll().await;
            tx1.send(Message::MqttEvent(event)).unwrap();
        }
    });

    let tx2 = tx.clone();
    tokio::spawn(async move {
        loop {
            sensor_timer.tick().await;
            tx2.send(Message::TickEvent).unwrap();
        }
    });

    loop {
        select! {
            _ = tokio::signal::ctrl_c() => {
                let _ = mqtt::announce_offline(&client).await;
                return;
            },
            event = rx.recv() => match event {
                Some(Message::MqttEvent(Ok(Event::Incoming(Incoming::ConnAck(_))))) => {
                    println!("MQTT connection ack incoming: announcing capabilities");
                    mqtt::announce_online(&client).await.unwrap();
                    mqtt::register_devices(&client, &payloads).await.unwrap();
                },
                Some(Message::MqttEvent(Ok(Event::Incoming(Incoming::Publish(Publish { topic, payload, .. }))))) => {
                    println!("MQTT command incoming: topic '{topic}' payload '{payload:?}'");
                    if let Some(cover) = covers.get(&topic) {
                        let payload = String::from_utf8(payload.to_vec()).unwrap();
                        let cmd: Result<CoverCommand, _> = payload.parse();
                        let c = cover.clone();

                        if let Ok(cmd) = cmd {
                            let tt = transmission_timeout.clone();

                            tokio::spawn(async move {
                                let mut t = tt.lock().await;
                                tokio::time::sleep_until((*t).into()).await;

                                let t1 = Instant::now();

                                match cmd {
                                    CoverCommand::Open => {
                                        c.move_up().await.unwrap();
                                    }
                                    CoverCommand::Close => {
                                        c.move_down().await.unwrap();
                                    }
                                    CoverCommand::Stop => {
                                        c.stop().await.unwrap();
                                    }
                                }

                                let elapsed = Instant::now() - t1;
                                *t = Instant::now() + (Duration::from_millis(config.global_tx_timeout_ms) - elapsed);
                            });
                        } else {
                            eprintln!("Err: invalid payload {payload:?}");
                        }
                    } else {
                        eprintln!("Err: unknown device at {topic}");
                    }
                },
                Some(Message::MqttEvent(Err(ConnectionError::Io(e)))) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                    eprintln!("Err(Io({e:?})): retrying in 10s");
                    tokio::time::sleep(Duration::from_secs(10)).await;
                },
                Some(Message::TickEvent) => {
                    for (topic, sunspec) in &mut sunspec_devices {
                        match sunspec.measure().await {
                            Ok(measurements) => mqtt::publish_state(&client, topic, &measurements).await.unwrap(),
                            Err(e) => eprintln!("Error: Unable to read from sunspec modbus: {e}"),
                        }
                    }
                }
                _ => (),
            }
        }
    }
}
