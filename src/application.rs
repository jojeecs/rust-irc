use std::io::{stdin, stdout, Write};
use std::net::Ipv4Addr;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;
use tokio::net::TcpStream;

#[derive(Clone)]
#[derive(Debug)]
pub struct Connection {
    pub(crate) ip: Ipv4Addr,
    pub port: u16
}

#[derive(Debug)]
pub struct Message {
    pub dst: Connection,
    pub src: Connection,
    pub contents: String,
}

pub async fn application(receiver: Receiver<TcpStream>) {
    loop {
        let stream = receiver.recv();
    }
}