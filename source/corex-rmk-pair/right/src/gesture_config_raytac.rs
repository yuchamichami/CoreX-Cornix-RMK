//! Keep generated behavior, moving only the cell occupied by the A14 encoder.
mod base {
    include!("gesture_config.rs");
}

pub const CONFIG: rmk::input_device::pointing::gesture::Config =
    rmk::input_device::pointing::gesture::Config {
        cells: [(5, 0, 6), (5, 1, 6), (5, 3, 6), (6, 0, 6), (6, 1, 6)],
        ..base::CONFIG
    };
