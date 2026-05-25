use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::ops::Add;
use std::sync::{mpsc, Arc, Mutex};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use rand::{Rng};
use common::{Client, Message, Server, Signal, DISCONNECT_DELIMITER, USERNAME_DELIMITER};
use common::Signal::*;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:8080").unwrap();

    let senders: Arc<Mutex<Vec<Client>>> = Arc::new(Mutex::new(Vec::new()));

    let (server_send, server_recv) = mpsc::channel::<Message>();

    let mut server = Server { capacity: 10, user_amount: 0, receiver: server_recv, clients: Arc::clone(&senders) };

    let senders_ref = Arc::clone(&senders);

    let mut user_count = 0;

    thread::spawn(move || {
        loop {
            if let Ok(mut message) = server.receiver.recv() {
                let mut clients = senders_ref.lock().unwrap();
                match message.signal {
                    None => {
                        for client in clients.iter() {
                            if client.id == message.owner.id {
                                continue;
                            }
                            message.contents = format!("{}: {}", message.owner.username.to_string(), message.contents);
                            match client.sender.send(message.clone()) {
                                Ok(_) => {}
                                Err(_) => {
                                }
                            }
                        }
                    }
                    Some(signal) => {
                        if signal == Disconnect {
                            let mut idx = 0;
                            let owner = message.owner;
                            println!("Client disconnecting: {} with ID {}", owner.username, owner.id);
                            let mut owner_found = false;
                            for client in clients.iter() {
                                if client.id == owner.id {
                                    owner_found = true;
                                } else {
                                    client.sender.send(Message { contents: format!("User {} Disconnected", owner.username.to_string()).to_string(),
                                        owner: Arc::clone(&owner), message_id: message.message_id, signal: None}).unwrap();
                                    if !owner_found {
                                        idx += 1;
                                    }
                                }
                            }
                            clients.remove(idx);
                        }
                    }
                }
            }
        }
    });
    
    while let Ok((stream, _)) = listener.accept() {
        let (sender, receiver) = Server::create_connection();
        user_count = server.user_amount;
        let client = Server::request_connection(sender, String::from("hello"), user_count).unwrap();
        senders.lock().unwrap().push(client.clone());
        let reader_stream = stream.try_clone().unwrap();
        let server_send_clone = server_send.clone();
        thread::spawn(move || {
            reader_socket(reader_stream, server_send_clone, client);
        });
        thread::spawn(move || {
            writer_socket(receiver, stream);
        });
    }
}


fn reader_socket(mut stream: TcpStream, server_sender: Sender<Message>, mut client: Client) {
    let mut buffer = vec![0; 1024];
    loop {
        let client_ref = Arc::new(client.clone());
        let byte_val = stream.read(&mut buffer).unwrap();
        if byte_val == 0 {
            server_sender.send(Message { contents: String::new(), owner:  client_ref, message_id: 0, signal: Some(Disconnect)}).unwrap();
            break;
        }
        let message = String::from_utf8_lossy(&buffer[0..byte_val - 1]).to_string();

        if message.starts_with(USERNAME_DELIMITER) {
            println!("{message}");
            let username = &message[USERNAME_DELIMITER.as_bytes().len() + 1..byte_val - 1];
            client.username = username.to_string();
        } else if message.starts_with(DISCONNECT_DELIMITER) {

        }
        else {
            let mut message = Message { contents: message, owner: client_ref, message_id: rand::rng().next_u64() as usize, signal: None };
            message.signal = message.parse_signal();
            server_sender.send(message).unwrap();
        }
    }
}

fn writer_socket(receiver: Receiver<Message>, mut stream: TcpStream) {
    loop {
        if let Ok(message) = receiver.recv() {
            match stream.write_all(message.contents.as_bytes()) {
                Ok(_) => {
                } Err(_) => {
                },
            }
        } else {
            break;
        }
    }
}

