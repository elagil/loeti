pub mod app;
pub mod comm;
pub mod data;
pub mod plotting;
pub use ergot::toolkits::nusb_v0_1 as kit;

const MTU: u16 = 128;
const OUT_BUFFER_SIZE: usize = 1024;
