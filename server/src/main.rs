use std::thread;
use std::time::Duration;
use crate::server::Server;

pub mod server;

#[tokio::main]
async fn main() {
    thread::spawn(async || {
        Server::run().await;
    }).join().unwrap().await;

    loop {
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
