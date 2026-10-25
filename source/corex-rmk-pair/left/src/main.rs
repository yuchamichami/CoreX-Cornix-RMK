#![no_std]
#![no_main]
#[path = "../../shared/status_led.rs"]
mod status_led;
use rmk::macros::rmk_peripheral;
#[rmk_peripheral(id = 0)]
mod keyboard {
    #[register_processor(poll)]
    async fn status_leds() -> crate::status_led::StatusLeds {
        use embassy_nrf::gpio::{Level, Output, OutputDrive};
        #[cfg(feature = "raytac-l-rev1")]
        {
            // Rev1.0: two common-anode RGB LEDs; HIGH means off.
            let lights = crate::status_led::Lights::Raytac {
                first: [
                    Output::new(p.P0_19, Level::High, OutputDrive::Standard),
                    Output::new(p.P0_16, Level::High, OutputDrive::Standard),
                    Output::new(p.P0_25, Level::High, OutputDrive::Standard),
                ],
                second: [
                    Output::new(p.P0_07, Level::High, OutputDrive::Standard),
                    Output::new(p.P0_27, Level::High, OutputDrive::Standard),
                    Output::new(p.P0_23, Level::High, OutputDrive::Standard),
                ],
            };
            let battery_enable = Output::new(p.P1_10, Level::High, OutputDrive::Standard);
            // Registered initializers finish before the generated ADC task.
            // R7/R8/C10 settle first; retain Output to keep the rail enabled.
            embassy_time::Timer::after_millis(350).await;
            crate::status_led::StatusLeds::new(lights, false)
                .with_battery_enable(battery_enable)
        }
        #[cfg(not(feature = "raytac-l-rev1"))]
        {
            use embassy_nrf::pwm::{Config, Prescaler, SequenceLoad, SequencePwm};
            // Verified stock Cornix left RGB: P0.13 power, P0.24 data, two GRB pixels.
            // PWM0 generates the waveform with EasyDMA, independently of BLE IRQs.
            let mut config = Config::default();
            config.prescaler = Prescaler::Div1;
            config.max_duty = 20;
            config.sequence_load = SequenceLoad::Common;
            config.ch0_idle_level = Level::Low;
            crate::status_led::StatusLeds::new(
                crate::status_led::Lights::Left {
                    power: Output::new(p.P0_13, Level::Low, OutputDrive::Standard),
                    pwm: SequencePwm::new_1ch(p.PWM0, p.P0_24, config).unwrap(),
                },
                false,
            )
        }
    }
}
