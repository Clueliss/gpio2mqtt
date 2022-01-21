mod config;
mod covers;
mod mqtt;

use std::{collections::HashMap, fs::File, time::Duration};

use covers::CoverCommand;
use rumqttc::{AsyncClient, ConnectionError, Event, Incoming, MqttOptions, Publish};
use tokio::select;

use crate::mqtt::ConfigurationPayload;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let config: config::Config =
        serde_yaml::from_reader(File::open("/etc/gpio2mqtt.yaml").unwrap()).unwrap();

    let covers: HashMap<_, _> = config
        .covers
        .iter()
        .map(|cover_conf| {
            let opts = covers::stateless_gpio::Options::from_chip_offsets(
                &cover_conf.chip,
                cover_conf.up_pin,
                cover_conf.down_pin,
                cover_conf.stop_pin,
            )
            .unwrap();

            (
                mqtt::command_topic_for_dev_id(&cover_conf.device.identifier),
                covers::stateless_gpio::Cover::new(opts),
            )
        })
        .collect();

    let payloads: Vec<ConfigurationPayload> = config
        .covers
        .into_iter()
        .map(mqtt::ConfigurationPayload::from)
        .collect();

    let opts = MqttOptions::new("gpio2mqtt_bridge", config.host, config.port);
    let (client, mut eventloop) = AsyncClient::new(opts, 10);

    loop {
        select! {
            event = eventloop.poll() => {
                match event {
                    Ok(Event::Incoming(Incoming::ConnAck(_))) => {
                        println!("Ok(Incoming(ConnAck(_))): announcing capabilities");
                        mqtt::announce_online(&client).await.unwrap();
                        mqtt::register_covers(&client, &payloads).await.unwrap();
                    },
                    Ok(Event::Incoming(Incoming::Publish(Publish { topic, payload, .. }))) => {
                        println!("Ok(Incoming(Publish {{ topic: {topic}, payload: {payload:?}, .. }}))");
                        if let Some(cover) = covers.get(&topic) {
                            let payload = String::from_utf8(payload.to_vec()).unwrap();
                            let cmd: Result<CoverCommand, _> = payload.parse();

                            match cmd {
                                Ok(CoverCommand::Open) => {
                                    cover.move_up().await.unwrap();
                                }
                                Ok(CoverCommand::Close) => {
                                    cover.move_down().await.unwrap();
                                }
                                Ok(CoverCommand::Stop) => {
                                    cover.stop().await.unwrap();
                                }
                                _ => eprintln!("Err: invalid payload {payload:?}"),
                            }
                        } else {
                            eprintln!("Err: unknown cover at {topic}");
                        }
                    },
                    Err(ConnectionError::Io(e)) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                        eprintln!("Err(Io({e:?})): retrying in 10s");
                        tokio::time::sleep(Duration::from_secs(10)).await;
                    },
                    other => println!("{other:?}"),
                }
            },
            _ = tokio::signal::ctrl_c() => {
                let _ = mqtt::announce_offline(&client).await;
                return;
            }
        }
    }
}
