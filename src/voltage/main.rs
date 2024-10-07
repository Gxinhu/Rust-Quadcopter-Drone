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
    use bsp::board;
    use bsp::{
        hal::{gpio, iomuxc},
        pins,
    };
    use teensy4_bsp as bsp;
    use teensy4_bsp::hal::adc::AnalogInput;

    use imxrt_log as logging;

    // If you're using a Teensy 4.1 or MicroMod, you should eventually
    // change 't40' to 't41' or micromod, respectively.
    use board::t40 as my_board;

    use rtic_monotonics::systick::{Systick, *};
    type VoltageRead = gpio::Input<pins::t40::P15>;
    const PIN_CONFIG: iomuxc::Config =
        iomuxc::Config::zero().set_pull_keeper(Some(iomuxc::PullKeeper::Pulldown100k));

    /// There are no resources shared across tasks.
    #[shared]
    struct Shared {}

    /// These resources are local to individual tasks.
    #[local]
    struct Local {
        /// The LED on pin 13.
        /// A poller to control USB logging.
        voltage_input: AnalogInput<pins::t40::P15, 1>,
        poller: logging::Poller,
        adc1: bsp::hal::adc::Adc<1>,
        led: bsp::board::Led
    }

    #[init]
    fn init(cx: init::Context) -> (Shared, Local) {
        let board::Resources {
            mut gpio2,
            pins,
            usb,
            adc1,
            ..
        } = my_board(cx.device);
        let voltage_input = AnalogInput::new(pins.p15);
        let led=bsp::board::led(&mut gpio2,pins.p13);
        let poller = logging::log::usbd(usb, logging::Interrupts::Enabled).unwrap();

        Systick::start(
            cx.core.SYST,
            board::ARM_FREQUENCY,
            rtic_monotonics::create_systick_token!(),
        );

        voltage_read::spawn().unwrap();
        (
            Shared {},
            Local {
                voltage_input,
                adc1,
                poller,
                led,
            },
        )
    }

    #[task(local = [voltage_input,adc1,led])]
    async fn voltage_read(cx: voltage_read::Context) {
        let adc1 = cx.local.adc1;
        let voltage_input = cx.local.voltage_input;
        let led = cx.local.led;
        loop {
            Systick::delay(2500.millis()).await;
            let voltage = adc1.read_blocking(voltage_input);
            let voltage = voltage as f32;
            log::info!("The voltage is {} V", voltage / 62.0);
            led.toggle();
        }
    }

    #[task(binds = USB_OTG1, local = [poller])]
    fn log_over_usb(cx: log_over_usb::Context) {
        cx.local.poller.poll();
    }
}
