//! The starter code slowly blinks the LED and sets up
//! USB logging. It periodically logs messages over USB.
//!
//! Despite targeting the Teensy 4.0, this starter code
//! should also work on the Teensy 4.1 and Teensy MicroMod.
//! You should eventually target your board! See inline notes.
//!
//! This template uses [RTIC v2](https://rtic.rs/2/book/en/)
//! for structuring the application.

#![no_std]
#![no_main]

use teensy4_panic as _;
#[rtic::app(device = teensy4_bsp, peripherals = true, dispatchers = [KPP])]
mod app {

    use imxrt_log as logging;
    use mpu6050::{Mpu6050, Mpu6050Error};
    use teensy4_bsp::{
        self as bsp,
        board::{self, Lpi2c1},
        hal::lpi2c,
    };
    // If you're using a Teensy 4.1 or MicroMod, you should eventually
    // change 't40' to 't41' or micromod, respectively.
    use board::t40 as my_board;

    /// There are no resources shared across tasks.
    #[shared]
    struct Shared {}

    /// These resources are local to individual tasks.
    #[local]
    struct Local {
        /// The LED on pin 13.
        /// A poller to control USB logging.
        poller: logging::Poller,
        mpu: Mpu6050<Lpi2c1>,
        delay: cortex_m::delay::Delay,
    }
    fn init_mpu(
        mpu: &mut Mpu6050<Lpi2c1>,
        delay: &mut cortex_m::delay::Delay,
    ) -> Result<(), Mpu6050Error<lpi2c::ControllerStatus>> {
        mpu.init(delay)?;
        mpu.set_gyro_range(mpu6050::device::GyroRange::D500)?;

        mpu.write_bits(
            mpu6050::device::CONFIG::ADDR,
            mpu6050::device::CONFIG::DLPF_CFG.bit,
            mpu6050::device::CONFIG::DLPF_CFG.length,
            5 as u8,
        )?;
        Ok(())
    }
    #[init]
    fn init(cx: init::Context) -> (Shared, Local) {
        let board::Resources {
            lpi2c1,
            mut gpio2,
            pins,
            usb,
            ..
        } = my_board(cx.device);

        let led = bsp::board::led(&mut gpio2, pins.p13);
        led.set();

        let lpi2c: board::Lpi2c1 =
            board::lpi2c(lpi2c1, pins.p19, pins.p18, board::Lpi2cClockSpeed::KHz400);
        let poller = logging::log::usbd(usb, logging::Interrupts::Enabled).unwrap();
        let mut mpu = Mpu6050::new(lpi2c);
        let mut delay = cortex_m::delay::Delay::new(cx.core.SYST, board::ARM_FREQUENCY);
        init_mpu(&mut mpu, &mut delay).expect("init mpu OK");

        voltage_read::spawn().unwrap();
        (Shared {}, Local { poller, mpu, delay })
    }

    #[task(local = [mpu,delay])]
    async fn voltage_read(cx: voltage_read::Context) {
        let mpu = cx.local.mpu;
        let delay = cx.local.delay;
        let mut rate_calibration_number = 0;
        let mut rate_calibration_roll = 0.0;
        let mut rate_calibration_pitch = 0.0;
        let mut rate_calibration_yaw = 0.0;
        while rate_calibration_number < 2000 {
            let gyro = mpu.get_gyro().expect("Get excepted")/mpu6050::PI_180;
            rate_calibration_roll += gyro[0];
            rate_calibration_pitch += gyro[1];
            rate_calibration_yaw += gyro[2];
            rate_calibration_number += 1;
            delay.delay_ms(1);
        }
        let rate_calibration_number = rate_calibration_number as f32;
        rate_calibration_roll /= rate_calibration_number;
        rate_calibration_pitch /= rate_calibration_number;
        rate_calibration_yaw /= rate_calibration_number;
        log::info!(
            "calibratrion gyro is {},{},{}",
            rate_calibration_roll,
            rate_calibration_pitch,
            rate_calibration_yaw
        );
        loop {
            delay.delay_ms(50);
            let acc = mpu.get_acc().expect("Get expected");
            log::info!("acc is {:?}", acc);
            let gyro = mpu.get_gyro().expect("Get excepted") / mpu6050::PI_180;
            log::info!(
                "gyro is {},{},{}",
                gyro[0] - rate_calibration_roll,
                gyro[1] - rate_calibration_pitch,
                gyro[2] - rate_calibration_yaw
            );
        }
    }

    #[task(binds = USB_OTG1, local = [poller])]
    fn log_over_usb(cx: log_over_usb::Context) {
        cx.local.poller.poll();
    }
}
