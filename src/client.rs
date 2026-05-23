use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpStream};
use std::str::FromStr;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use crate::application::{Connection, Message};

pub struct Client {
    ip: Ipv4Addr,
    nickname: String,
    port: u16,
}

impl Client {
    pub async fn connect(sender: Sender<Message>, mut receiver: Receiver<Message>) {
        tokio::spawn(async move {
            let stream = TcpStream::connect("127.0.0.1:8080").unwrap();
            let this = Connection { ip: Ipv4Addr::from_str(stream.local_addr().unwrap().ip().to_string().as_str()).unwrap(), port: stream.local_addr().unwrap().port() };

            loop {
                let reader = BufReader::new(&stream);
                let received = receiver.recv().await.unwrap();
                if let Some(line) = reader.lines().next() {
                    println!("{:?}", line.unwrap().lines());
                }
                println!("Received message from server: {received:?}");
                let server_connection = Connection { ip: Ipv4Addr::from_str("127.0.0.1").unwrap(), port: 8080 };
                let new_msg = Message { src: this.clone(), dst: server_connection, contents: String::from("Hello from client!") };
                sender.send(new_msg).await.unwrap();
            }
        });
    }
}