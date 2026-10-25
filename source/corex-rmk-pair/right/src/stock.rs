//! Stock Cornix right: keyboard/encoder only; no pointing GPIO is initialized.
use rmk::macros::rmk_central;

#[rmk_central]
mod keyboard {
    #[register_processor(poll)]
    fn pairing_keys() -> crate::pairing_keys::PairingKeys {
        crate::pairing_keys::PairingKeys::new()
    }

    #[register_processor(poll)]
    fn status_leds() -> crate::status_led::StatusLeds {
        use embassy_nrf::gpio::{Level, Output, OutputDrive};
        use embassy_nrf::pwm::{Config, Prescaler, SequenceLoad, SequencePwm};
        // Stock right swaps the left board's RGB power and data pins.
        let mut config = Config::default();
        config.prescaler = Prescaler::Div1;
        config.max_duty = 20;
        config.sequence_load = SequenceLoad::Common;
        config.ch0_idle_level = Level::Low;
        crate::status_led::StatusLeds::new(
            crate::status_led::Lights::Left {
                power: Output::new(p.P0_24, Level::Low, OutputDrive::Standard),
                pwm: SequencePwm::new_1ch(p.PWM0, p.P0_13, config).unwrap(),
            },
            true,
        )
    }
}
