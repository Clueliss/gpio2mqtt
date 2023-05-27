use crate::{covers, sunspec};
use std::{future::Future, sync::Arc};
use tokio::{
    sync::{mpsc, watch, Mutex},
    time::{self, Duration, Instant},
};

pub struct Pause {
    delay: Duration,
    delay_done: Option<Instant>,
}

impl Pause {
    pub fn new(delay: Duration) -> Self {
        Self { delay, delay_done: None }
    }

    pub fn reset(&mut self) {
        self.delay_done = Some(Instant::now() + self.delay);
    }

    pub async fn pause(&mut self) {
        if let Some(deadline) = self.delay_done.take() {
            time::sleep_until(deadline).await;
        }
    }
}

pub enum Message {
    SunspecMeasurement(String, sunspec::varta::Measurements),
    MqttEvent(paho_mqtt::Message),
}

pub fn mqtt_message_event_loop(
    mqtt_stream: paho_mqtt::AsyncReceiver<Option<paho_mqtt::Message>>,
    tx: mpsc::Sender<Message>,
) -> impl Future<Output = ()> {
    async move {
        loop {
            let Ok(event) = mqtt_stream.recv().await else {
                break;
            };

            match event {
                Some(event) => if let Err(_) = tx.send(Message::MqttEvent(event)).await {
                    break;
                },
                None => println!("Lost connection to server"),
            }
        }

        println!("Shutting down MQTT client");
    }
}

pub fn stateless_cover_event_loop(
    topic: String,
    group_gpio_pause: Arc<Mutex<Pause>>,
    device_gpio_pause: Duration,
    device: covers::stateless_gpio::Cover,
) -> (watch::Sender<covers::CoverCommand>, impl Future<Output = ()>) {
    let (tx, mut rx) = watch::channel(covers::CoverCommand::Stop);

    let fut = async move {
        while let Ok(_) = rx.changed().await {
            let mut gtt = group_gpio_pause.lock().await;
            gtt.pause().await;

            let cmd = *rx.borrow();

            if let Err(e) = device.issue_command(cmd).await {
                eprintln!("Error unable to set gpio pin: {e}");
            }

            gtt.reset();
            time::sleep(device_gpio_pause).await;
        }

        println!("Shutting down command listener for {topic}");
    };

    (tx, fut)
}

pub fn sunspec_event_loop(
    topic: String,
    device_polling_delay: Duration,
    mut device: sunspec::varta::ElementSunspecClient,
    tx: mpsc::Sender<Message>,
) -> impl Future<Output = ()> {
    let mut sensor_timer = time::interval(device_polling_delay);
    sensor_timer.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

    async move {
        let mut last_measurement = None;

        loop {
            sensor_timer.tick().await;

            match time::timeout(Duration::from_secs(5), device.measure()).await {
                Ok(Ok(measurement)) => {
                    last_measurement = Some(measurement);

                    if let Err(_) = tx.send(Message::SunspecMeasurement(topic.clone(), measurement)).await {
                        break;
                    }
                },
                Ok(Err(e)) => eprintln!("Error unable to read from sunspec modbus: {e}"),
                Err(elapsed) => {
                    eprintln!("Error modbus request for {topic} timed out after {elapsed}, trying again in 1 minute");

                    if let Some(last_measurement) = last_measurement.take() {
                        let placeholder = sunspec::varta::Measurements {
                            active_battery_power: None,
                            apparent_battery_power: None,
                            grid_power: None,
                            ..last_measurement
                        };

                        if let Err(_) = tx.send(Message::SunspecMeasurement(topic.clone(), placeholder)).await {
                            break;
                        }
                    }

                    time::sleep(Duration::from_secs(60)).await;
                },
            }
        }

        println!("Shutting down update timer for {topic}");
    }
}
