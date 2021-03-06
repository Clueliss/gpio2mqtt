use std::{
    path::Path,
    time::{Duration, Instant},
};

use crate::config::Identifier;
use gpio_cdev::{Chip, LineHandle, LineRequestFlags};
use tokio::{select, sync::Mutex};
use tokio_util::sync::CancellationToken;

use super::CoverCommand;

async fn gpio_sim_short_press(handle: &LineHandle) -> Result<(), gpio_cdev::Error> {
    handle.set_value(1)?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    handle.set_value(0)?;
    Ok(())
}

pub struct Options {
    pub up: LineHandle,
    pub down: LineHandle,
    pub stop: LineHandle,
    pub tx_timeout: Duration,
}

impl Options {
    pub fn from_chip_offsets<P: AsRef<Path>>(
        chip_path: P,
        up_offset: u32,
        down_offset: u32,
        stop_offset: u32,
        tx_timeout: Duration,
        identifier: &Identifier,
    ) -> Result<Self, gpio_cdev::Error> {
        let mut chip = Chip::new(chip_path)?;
        let up = chip.get_line(up_offset)?.request(
            LineRequestFlags::OUTPUT,
            0,
            &format!("gpio2mqtt_{}_up", &identifier.0),
        )?;
        let down = chip.get_line(down_offset)?.request(
            LineRequestFlags::OUTPUT,
            0,
            &format!("gpio2mqtt_{}_down", &identifier.0),
        )?;
        let stop = chip.get_line(stop_offset)?.request(
            LineRequestFlags::OUTPUT,
            0,
            &format!("gpio2mqtt_{}_stop", &identifier.0),
        )?;

        Ok(Self {
            up,
            down,
            stop,
            tx_timeout,
        })
    }
}

pub struct Cover {
    options: Options,
    timeout: Mutex<Instant>,
    state: Mutex<Option<(CoverCommand, CancellationToken)>>,
}

impl Cover {
    pub fn new(options: Options) -> Self {
        Self {
            options,
            timeout: Mutex::new(Instant::now()),
            state: Mutex::new(None),
        }
    }

    pub async fn issue_command(&self, action: CoverCommand) -> Result<(), gpio_cdev::Error> {
        let ctok = {
            let mut s = self.state.lock().await;

            match s.take() {
                Some((a, cancel)) if a != action => {
                    cancel.cancel();
                }
                Some(_) => {
                    return Ok(());
                }
                None => {}
            }

            let ctok = CancellationToken::new();
            *s = Some((action, ctok.clone()));

            ctok
        };

        let mut d = self.timeout.lock().await;

        select! {
            _ = ctok.cancelled() => {},
            _ = tokio::time::sleep_until((*d).into()) => {
                let line = match action {
                    CoverCommand::Open => &self.options.up,
                    CoverCommand::Close => &self.options.down,
                    CoverCommand::Stop => &self.options.stop,
                };

                gpio_sim_short_press(line).await?;
                *d = Instant::now() + self.options.tx_timeout;
            }
        }

        Ok(())
    }

    pub async fn move_up(&self) -> Result<(), gpio_cdev::Error> {
        self.issue_command(CoverCommand::Open).await
    }

    pub async fn move_down(&self) -> Result<(), gpio_cdev::Error> {
        self.issue_command(CoverCommand::Close).await
    }

    pub async fn stop(&self) -> Result<(), gpio_cdev::Error> {
        self.issue_command(CoverCommand::Stop).await
    }
}
