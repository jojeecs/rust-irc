use std::net::TcpStream;
use std::str::FromStr;
use std::thread;
use std::time::Duration;
use tokio::sync::mpsc;
use crate::application::Message;
use crate::client::Client;
use crate::server::Server;
use tokio::sync::mpsc::{Receiver, Sender};

pub mod server;
pub mod client;
pub mod application;

#[tokio::main]
async fn main() {
    let (con_sender, mut client_recver) = mpsc::channel::<Message>(10);
    let (msg_sender, mut msg_recver) = mpsc::channel::<Message>(10);
    thread::spawn(async || {
        Server::start(con_sender, msg_recver).await;
    }).join().unwrap().await;


    tokio::time::sleep(Duration::from_millis(100)).await;

    thread::spawn(async || {
        Client::connect(msg_sender, client_recver).await;
    }).join().unwrap().await;

    loop {
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
