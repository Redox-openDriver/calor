#![no_std]

extern crate alloc;

pub mod cooling;
pub mod reading;
pub mod watch;

pub use cooling::{Change, Cooling, Sample, assess};
pub use reading::{Place, Reading, Sensor};
pub use watch::{Limits, Notice, Watch};
