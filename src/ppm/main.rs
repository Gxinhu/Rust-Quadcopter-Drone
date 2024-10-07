#![no_std]
#![no_main]

use teensy4_panic as _;

#[rtic::app(device = teensy4_bsp, peripherals = true, dispatchers = [KPP])]
mod app {
    use bsp::board;
    use bsp::{hal::iomuxc, pins};
    use imxrt_log as logging;
    use rtic_monotonics::{
        systick::{self, Systick},
        Monotonic,
    };
    use teensy4_bsp as bsp;
    // If you're using a Teensy 4.1 or MicroMod, you should eventually
    // change 't40' to 't41' or micromod, respectively.
    use board::t40 as my_board;
    const PIN_CONFIG: iomuxc::Config = iomuxc::Config::zero()
        .set_pull_keeper(Some(iomuxc::PullKeeper::Pullup22k))
        .set_open_drain(pins::OpenDrain::Enabled);
    /// There are no resources shared across tasks.
    #[shared]
    struct Shared {}

    /// These resources are local to individual tasks.
    #[local]
    struct Local {
        led: board::Led,
        poller: logging::Poller,
        ppm: bsp::hal::gpio::Input<bsp::pins::t40::P14>,
        last_time: u32,
        ppm_data: [u16; 9], // 假设有8个通道
        channel: usize,
    }

    #[init]
    fn init(cx: init::Context) -> (Shared, Local) {
        let board::Resources {
            mut gpio2,
            mut gpio1,
            mut pins,
            usb,
            ..
        } = my_board(cx.device);
        let led = board::led(&mut gpio2, pins.p13);
        let poller = logging::log::usbd(usb, logging::Interrupts::Enabled).unwrap();
        iomuxc::configure(&mut pins.p14, PIN_CONFIG);
        let ppm = gpio1.input(pins.p14);

        gpio1.set_interrupt(&ppm, Some(bsp::hal::gpio::Trigger::RisingEdge));
        let ppm_data = [0u16; 9];
        let channel = 0;
        led.set();
        Systick::start(
            cx.core.SYST,
            board::ARM_FREQUENCY / 100,
            rtic_monotonics::create_systick_token!(),
        );
        let last_time = 0;
        log::info!("PPM Data: {:?}", last_time);
        (
            Shared {},
            Local {
                led,
                poller,
                ppm,
                ppm_data,
                channel,
                last_time,
            },
        )
    }
    #[task(binds=GPIO1_COMBINED_16_31, local=[led, ppm, last_time, ppm_data, channel])]
    fn ppm_interrupt(cx: ppm_interrupt::Context) {
        let now = Systick::now().ticks();
        let elapsed = now - *cx.local.last_time;
        *cx.local.last_time = now;
        if *cx.local.channel < cx.local.ppm_data.len() {
            cx.local.ppm_data[*cx.local.channel] = elapsed as u16;
            *cx.local.channel += 1;
            if elapsed > 2100 || *cx.local.channel >= cx.local.ppm_data.len() {
                *cx.local.channel = 0;
                log::info!("PPM Data: {:?}", cx.local.ppm_data);
            }
        }

        cx.local.ppm.clear_triggered();
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            cortex_m::asm::wfi();
        }
    }

    #[task(binds = USB_OTG1, local = [poller])]
    fn log_over_usb(cx: log_over_usb::Context) {
        cx.local.poller.poll();
    }
}
