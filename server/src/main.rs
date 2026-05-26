use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::ops::Add;
use std::sync::{mpsc, Arc, Mutex};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use rand::{rng, Rng};
use common::{Action, Client, Message, Server, ServerDB, Signal, DISCONNECT_DELIMITER, USERNAME_DELIMITER};
use common::Action::{UserConnect, UserDisconnect};
use common::Signal::*;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:8080").unwrap();

    let senders: Arc<Mutex<Vec<Client>>> = Arc::new(Mutex::new(Vec::new()));

    let (server_send, server_recv) = mpsc::channel::<Message>();

    let (db_send, db_recv) = mpsc::channel::<Action>();

    let server = Server { receiver: server_recv, clients: Arc::clone(&senders) };

    let server_db = Arc::new(Mutex::new( ServerDB { capacity: 10, user_count: 0 }));

    let db_ref = Arc::clone(&server_db);

    let senders_ref = Arc::clone(&senders);

    thread::spawn(move || {
        loop {
            if let Ok(signal) = db_recv.recv() {
                match signal {
                    UserDisconnect => {
                        db_ref.lock().unwrap().dec_user_count();
                    },
                    UserConnect => {
                        db_ref.lock().unwrap().inc_user_count();
                    }
                }
            }
        }
    });

    thread::spawn(move || {
        loop {
            if let Ok(mut message) = server.receiver.recv() {
                let mut clients = senders_ref.lock().unwrap();
                match Message::parse_signal(message.contents.clone()) {
                    None => {
                        for client in clients.iter() {
                            if client.id == message.owner.id {
                                continue;
                            }
                            if message.contents.is_empty() {
                                continue;
                            }
                            message.contents = format!("{}: {}", message.owner.username.to_string().trim_ascii(), message.contents.trim_ascii());
                            match client.sender.send(message.clone()) {
                                Ok(_) => {}
                                Err(_) => {
                                }
                            }
                        }
                    }
                    Some(signal) => {
                        if signal == Disconnect {
                            db_send.send(UserDisconnect).unwrap();
                            let mut idx = 0;
                            let owner = message.owner;
                            println!("{} disconnecting", owner.id);
                            let mut owner_found = false;
                            for client in clients.iter() {
                                if client.id == owner.id {
                                    owner_found = true;
                                } else {
                                    client.sender.send(Message { contents: format!("User {} Disconnected", owner.username.to_string().trim_ascii()).to_string(),
                                        owner: Arc::clone(&owner), message_id: message.message_id, signal: None}).unwrap();
                                    if !owner_found {
                                        idx += 1;
                                    }
                                }
                            }
                            clients.remove(idx);
                        } else if signal == Connect {
                            db_send.send(UserConnect).unwrap();
                        }
                    }
                }
            }
        }
    });
    
    while let Ok((stream, _)) = listener.accept() {

        let new_user_id = rng().next_u64();

        println!("User with ID {new_user_id} connected");

        let (sender, receiver) = Server::create_connection();
        let client = Server::request_connection(sender, String::new(), new_user_id as usize).unwrap();

        server_send.send(Message { contents: String::new(), owner: Arc::new(client.clone()), message_id: 0, signal: Some(Connect) }).unwrap();

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
        let message = String::from_utf8_lossy(&buffer[0..byte_val]).to_string();

        if message.starts_with(USERNAME_DELIMITER) {
            let username = &message[USERNAME_DELIMITER.as_bytes().len() + 1..byte_val];
            client.username = username.to_string();
        } else {
            let message = Message { contents: message.clone(), owner: client_ref, message_id: rand::rng().next_u64() as usize, signal: Message::parse_signal(message.clone()) };
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

