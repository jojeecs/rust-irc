use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpStream};
use std::str::FromStr;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use ::client::{Connection, Message};
use std::net::{SocketAddr, TcpListener};
use socket2::{Socket, Domain, Type, SockAddr};

pub struct Client {
    ip: Ipv4Addr,
    nickname: String,
    port: u16,
}

impl Client {
    pub async fn connect() {
        let stream = TcpStream::connect("127.0.0.1:8080").unwrap();
        let this = Connection { ip: Ipv4Addr::from_str(stream.local_addr().unwrap().ip().to_string().as_str()).unwrap(), port: stream.local_addr().unwrap().port() };

        loop {
            let reader = BufReader::new(&stream);
            if let Some(line) = reader.lines().next() {
                println!("{:?}", line.unwrap().lines());
            }
            let server_connection = Connection { ip: Ipv4Addr::from_str("127.0.0.1").unwrap(), port: 8080 };
            let new_msg = Message { src: this.clone(), dst: server_connection, contents: String::from("Hello from client!") };
        }
    }

    pub fn send_msg() {

    }
}