use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, TcpStream};
use std::net::TcpListener;
use std::rc::Rc;
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use tokio::io::AsyncWriteExt;
use common::{Message, Connection};

pub struct Server {
    pub connections: Vec<TcpStream>,
    pub message_store: Vec<Message>
}

impl Server {
    pub async fn run() {
        tokio::spawn(async move {
            let listener = TcpListener::bind("127.0.0.1:8080").unwrap();
            let this = Connection { ip: Ipv4Addr::new(127, 0, 0, 1), port: 8080 };
            let mut handles: Vec<JoinHandle<()>> = Vec::new();

            for stream in listener.incoming() {
                let stream = stream.unwrap();


                let handle = thread::spawn(move|| {
                    for line in BufReader::new(stream).lines() {
                        println!("{}", line.unwrap());
                    }
                });
            }
        });
    }

    pub async fn process_new_msg(&mut self, message: Message) {
        println!("Storing new message: {message:?}");
        self.message_store.push(message);
        println!("Successfully stored message.");
    }
}