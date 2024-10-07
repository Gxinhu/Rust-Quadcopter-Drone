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

    use imxrt_log as logging;

    // If you're using a Teensy 4.1 or MicroMod, you should eventually
    // change 't40' to 't41' or micromod, respectively.
    use board::t40 as my_board;

    use rtic_monotonics::systick::{Systick, *};
    type RedOutput = gpio::Output<pins::t40::P5>;
    type GreenOutput = gpio::Output<pins::t40::P6>;

    /// There are no resources shared across tasks.
    #[shared]
    struct Shared {}

    /// These resources are local to individual tasks.
    #[local]
    struct Local {
        /// The LED on pin 13.
        led: board::Led,
        red: RedOutput,
        green: GreenOutput,
        /// A poller to control USB logging.
        poller: logging::Poller,
    }

    #[init]
    fn init(cx: init::Context) -> (Shared, Local) {
        let board::Resources {
            mut gpio4,
            mut gpio2,
            pins,
            usb,
            ..
        } = my_board(cx.device);
        // let led = board::led(&mut gpio2, pins.p13);
        //
        let led = gpio2.output(pins.p13);
        // iomuxc::configure(&mut pins.p5, PIN_CONFIG);
        let red = gpio4.output(pins.p5);
        // iomuxc::configure(&mut pins.p6, PIN_CONFIG);
        let green = gpio2.output(pins.p6);
        let poller = logging::log::usbd(usb, logging::Interrupts::Enabled).unwrap();

        Systick::start(
            cx.core.SYST,
            board::ARM_FREQUENCY,
            rtic_monotonics::create_systick_token!(),
        );

        blink::spawn().unwrap();
        (
            Shared {},
            Local {
                led,
                red,
                green,
                poller,
            },
        )
    }

    #[task(local = [led,red,green])]
    async fn blink(cx: blink::Context) {
        cx.local.led.set();
        cx.local.green.set();
        Systick::delay(2500.millis()).await;
        cx.local.green.clear();
        cx.local.red.set();
    }

    #[task(binds = USB_OTG1, local = [poller])]
    fn log_over_usb(cx: log_over_usb::Context) {
        cx.local.poller.poll();
    }
}
