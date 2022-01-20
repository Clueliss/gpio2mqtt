use std::{path::Path, time::Duration};

use gpio_cdev::{Chip, Line, LineHandle, LineRequestFlags};

async fn gpio_sim_short_press(handle: &LineHandle) -> Result<(), gpio_cdev::Error> {
    handle.set_value(1)?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    handle.set_value(0)?;
    Ok(())
}

pub struct Options {
    pub up: Line,
    pub down: Line,
    pub stop: Line,
}

impl Options {
    pub fn from_chip_offsets<P: AsRef<Path>>(
        chip_path: P,
        up_offset: u32,
        down_offset: u32,
        stop_offset: u32,
    ) -> Result<Self, gpio_cdev::Error> {
        let mut chip = Chip::new(chip_path)?;
        let up = chip.get_line(up_offset)?;
        let down = chip.get_line(down_offset)?;
        let stop = chip.get_line(stop_offset)?;

        Ok(Self { up, down, stop })
    }
}

pub struct Cover {
    options: Options,
}

impl Cover {
    pub fn new(options: Options) -> Self {
        Self { options }
    }

    pub async fn move_up(&self) -> Result<(), gpio_cdev::Error> {
        let up_h = self
            .options
            .up
            .request(LineRequestFlags::OUTPUT, 0, "cover-up")?;
        gpio_sim_short_press(&up_h).await?;
        Ok(())
    }

    pub async fn move_down(&self) -> Result<(), gpio_cdev::Error> {
        let down_h = self
            .options
            .down
            .request(LineRequestFlags::OUTPUT, 0, "cover-down")?;
        gpio_sim_short_press(&down_h).await?;
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), gpio_cdev::Error> {
        let stop_h = self
            .options
            .stop
            .request(LineRequestFlags::OUTPUT, 0, "cover-stop")?;
        gpio_sim_short_press(&stop_h).await?;
        Ok(())
    }
}
