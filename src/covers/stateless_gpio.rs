use super::CoverCommand;
use gpio_cdev::{Chip, LineHandle, LineRequestFlags};
use std::path::Path;
use tokio::time::Duration;

async fn gpio_sim_short_press(handle: &LineHandle) -> Result<(), gpio_cdev::Error> {
    handle.set_value(1)?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    handle.set_value(0)?;
    Ok(())
}

pub struct Cover {
    pub up: LineHandle,
    pub down: LineHandle,
    pub stop: LineHandle,
}

impl Cover {
    pub fn from_chip_offsets<P: AsRef<Path>>(
        chip_path: P,
        up_offset: u32,
        down_offset: u32,
        stop_offset: u32,
    ) -> Result<Self, gpio_cdev::Error> {
        const CONSUMER: &str = "gpio2mqtt";

        let mut chip = Chip::new(chip_path)?;

        let up = chip
            .get_line(up_offset)?
            .request(LineRequestFlags::OUTPUT, 0, CONSUMER)?;

        let down = chip
            .get_line(down_offset)?
            .request(LineRequestFlags::OUTPUT, 0, CONSUMER)?;

        let stop = chip
            .get_line(stop_offset)?
            .request(LineRequestFlags::OUTPUT, 0, CONSUMER)?;

        Ok(Self { up, down, stop })
    }

    pub async fn issue_command(&self, cmd: CoverCommand) -> Result<(), gpio_cdev::Error> {
        match cmd {
            CoverCommand::Open => self.move_up().await,
            CoverCommand::Close => self.move_down().await,
            CoverCommand::Stop => self.stop().await,
        }
    }

    pub async fn move_up(&self) -> Result<(), gpio_cdev::Error> {
        gpio_sim_short_press(&self.up).await
    }

    pub async fn move_down(&self) -> Result<(), gpio_cdev::Error> {
        gpio_sim_short_press(&self.down).await
    }

    pub async fn stop(&self) -> Result<(), gpio_cdev::Error> {
        gpio_sim_short_press(&self.stop).await
    }
}
