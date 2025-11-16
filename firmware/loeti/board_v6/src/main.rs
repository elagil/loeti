#![no_std]
#![no_main]

use embassy_executor::Spawner;
use loeti::app::app;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    spawner.must_spawn(app(spawner));
}
