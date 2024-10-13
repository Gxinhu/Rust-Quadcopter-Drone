#![no_std]
#![no_main]

use teensy4_panic as _;

#[rtic::app(device = teensy4_bsp, peripherals = true, dispatchers = [KPP])]
mod app {
    use bsp::board;
    use bsp::hal::flexpwm;
    use bsp::{hal::iomuxc, pins};
    use imxrt_log as logging;
    use rtic_monotonics::{systick::Systick, Monotonic};
    use teensy4_bsp as bsp;
    use teensy4_bsp::board::IPG_FREQUENCY;
    use teensy4_bsp::hal::flexpwm::Output;
    // If you're using a Teensy 4.1 or MicroMod, you should eventually
    // change 't40' to 't41' or micromod, respectively.
    use board::t40 as my_board;
    const PWM_PRESCALER: flexpwm::Prescaler = flexpwm::Prescaler::Prescaler64;
    const PWM_FREQUENCY: u32 = IPG_FREQUENCY / PWM_PRESCALER.divider();
    const PWM_HZ: u32 = 50;
    const MAX_RANGE: i16 = (PWM_FREQUENCY / PWM_HZ / 2) as i16;
    const MIN_RANGE: i16 = -MAX_RANGE;
    const MAX_DUTY: i16 = (MAX_RANGE as f32 / 10.0) as i16;
    const MIN_DUTY: i16 = -MAX_DUTY;
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
        ppm_data: [u16; 9],
        channel: usize,
        sm2: flexpwm::Submodule<1, 3>,
        output: Output<pins::t40::P7>,
        pwm: flexpwm::Pwm<1>,
    }

    #[init]
    fn init(cx: init::Context) -> (Shared, Local) {
        let board::Resources {
            mut gpio2,
            mut gpio1,
            mut pins,
            usb,
            flexpwm1,
            ..
        } = my_board(cx.device);

        let led = board::led(&mut gpio2, pins.p13);
        let poller = logging::log::usbd(usb, logging::Interrupts::Enabled).unwrap();
        iomuxc::configure(&mut pins.p14, PIN_CONFIG);
        let ppm = gpio1.input(pins.p14);
        let mut sm2 = flexpwm1.1 .3;
        let mut pwm = flexpwm1.0;
        // Keep running in wait, debug modes.
        sm2.set_debug_enable(true);
        sm2.set_wait_enable(true);
        // Run on the IPG clock.
        sm2.set_clock_select(flexpwm::ClockSelect::Ipg);
        // Divide the IPG clock by 1.
        sm2.set_prescaler(PWM_PRESCALER);
        // Allow PWM outputs to operate independently.
        sm2.set_pair_operation(flexpwm::PairOperation::Independent);
        // Reload every time the full reload value register compares.
        sm2.set_load_mode(flexpwm::LoadMode::reload_full());
        sm2.set_load_frequency(1);
        // Count over the full range of i16 values.
        sm2.set_initial_count(&pwm, MIN_RANGE);
        sm2.set_value(flexpwm::FULL_RELOAD_VALUE_REGISTER, MAX_RANGE);
        let output = flexpwm::Output::new_b(pins.p7);
        output.set_turn_on(&sm2, MIN_DUTY);
        output.set_turn_off(&sm2, 0);
        output.set_output_enable(&mut pwm, true);
        // Load the values into the PWM registers.
        sm2.set_load_ok(&mut pwm);
        // Start running.
        sm2.set_running(&mut pwm, true);
        gpio1.set_interrupt(&ppm, Some(bsp::hal::gpio::Trigger::FallingEdge));

        // Start running.
        let ppm_data = [0u16; 9];
        let channel = 0;
        led.set();
        Systick::start(
            cx.core.SYST,
            board::ARM_FREQUENCY / 100,
            rtic_monotonics::create_systick_token!(),
        );

        let last_time = 0;
        (
            Shared {},
            Local {
                led,
                poller,
                ppm,
                ppm_data,
                channel,
                last_time,
                sm2,
                pwm,
                output,
            },
        )
    }
    #[task(binds=GPIO1_COMBINED_16_31, local=[led, ppm, last_time, ppm_data, channel,output,sm2,pwm])]
    fn ppm_interrupt(cx: ppm_interrupt::Context) {
        let output = cx.local.output;
        let sm2 = cx.local.sm2;
        let pwm = cx.local.pwm;
        let now = Systick::now().ticks();
        let elapsed = now - *cx.local.last_time;
        *cx.local.last_time = now;
        if *cx.local.channel < cx.local.ppm_data.len() {
            cx.local.ppm_data[*cx.local.channel] = elapsed as u16;
            *cx.local.channel += 1;
            if elapsed > 2100 || *cx.local.channel >= cx.local.ppm_data.len() {
                *cx.local.channel = 0;
                if filters(cx.local.ppm_data) {
                    let input_throttle =
                        (((cx.local.ppm_data[2] - 1000) as f32 / 1000.0) * MAX_DUTY as f32) as i16;
                    output.set_turn_off(&sm2, input_throttle);
                    sm2.set_load_ok(pwm);
                }
            }
        }

        cx.local.ppm.clear_triggered();
    }
    fn filters(ppm_data: &[u16; 9]) -> bool {
        for i in 0..8 {
            let ppm = ppm_data[i];
            if ppm < 1000 || ppm > 2000 {
                return false;
            }
        }
        true
    }

    #[task(binds = USB_OTG1, local = [poller])]
    fn log_over_usb(cx: log_over_usb::Context) {
        cx.local.poller.poll();
    }
}
