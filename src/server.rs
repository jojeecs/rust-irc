use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::str::FromStr;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use crate::application::{application, Connection, Message};

pub struct Server {
    pub connections: Vec<ServerConnection>,
    pub message_store: Vec<Message>
}

pub struct ServerConnection {
    client_ip: Ipv4Addr,
}



impl Server {
    pub async fn start(sender: Sender<Message>, mut receiver: Receiver<Message>) {
        tokio::spawn(async move {
            let server = Server { connections: Vec::new(), message_store: Vec::new() };
            let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();
            let this = Connection { ip: Ipv4Addr::new(127, 0, 0, 1), port: 8080 };

            loop {

                if let Ok(socket) = listener.accept().await
                {
                    println!("Connection from {}", socket.1);
                    let connection = Connection { ip: Ipv4Addr::from_str(socket.1.ip().to_string().as_str()).unwrap(), port: socket.1.port() };
                    let msg = Message { dst: connection, src: this.clone(), contents: String::from("Welcome to the server") };
                    sender.send(msg).await.unwrap();
                }
                if let Some(received) = receiver.recv().await {
                    println!("Received message from client: {received:?}");
                }
            }
        });
    }

    pub async fn process_new_msg(&mut self, message: Message) {
        println!("Storing new message: {message:?}");
        self.message_store.push(message);
        println!("Successfully stored message.");
    }
}