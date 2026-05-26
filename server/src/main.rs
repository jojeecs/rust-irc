use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Arc, Mutex};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use rand::{rng, Rng};
use common::{Action, Client, ClientID, Message, Server, ServerDB, Tag, USERNAME_DELIMITER};
use common::Action::{UserConnect, UserDisconnect};
use common::Command::PM;
use common::Signal::*;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:8080").unwrap();

    let senders: Arc<Mutex<Vec<Client>>> = Arc::new(Mutex::new(Vec::new()));

    let mut occupied_ids: Vec<ClientID> = Vec::new();

    let (server_send, server_recv) = mpsc::channel::<Message>();

    let (db_send, db_recv) = mpsc::channel::<Action>();

    let server = Server { receiver: server_recv, clients: Arc::clone(&senders) };

    let server_db = Arc::new(Mutex::new(ServerDB { capacity: 10, user_count: 0 }));

    let senders_ref = Arc::clone(&senders);

    thread::spawn(move || {
        db_handler(server_db, db_recv);
    });

    thread::spawn(move || {
        server_handler(server, db_send, senders_ref);
    });

    while let Ok((stream, _)) = listener.accept() {
        let (sender, receiver) = Server::create_connection();

        let new_user_id = rng().next_u64();
        let mut client_id = ClientID::generate(String::new(), new_user_id as usize);

        if occupied_ids.contains(&client_id) {
            client_id = ClientID::generate(String::new(), new_user_id as usize);
        } else {
            occupied_ids.push(client_id.clone());
        }

        let client = Server::request_connection(sender, client_id).unwrap();
        let client_clone = client.clone();

        senders.lock().unwrap().push(client);

        let reader_stream = stream.try_clone().unwrap();
        let server_send_clone = server_send.clone();


        thread::spawn(move || {
            reader_socket(reader_stream, server_send_clone, client_clone);
        });

        thread::spawn(move || {
            writer_socket(receiver, stream);
        });
    }
}

fn reader_socket(mut stream: TcpStream, server_sender: Sender<Message>, mut client: Client) {
    let mut buffer = vec![0; 1024];
    loop {
        let client_reference = Arc::new(client.clone());
        let byte_val = stream.read(&mut buffer).unwrap();
        if byte_val == 0 {
            server_sender.send(Message { contents: String::new(), owner:  client_reference, message_id: 0, signal: Some(Disconnect)}).unwrap();
            break;
        }
        let message = String::from_utf8_lossy(&buffer[0..byte_val]).to_string();

        if message.starts_with(USERNAME_DELIMITER) {
            let username = &message[USERNAME_DELIMITER.as_bytes().len() + 1..byte_val];
            client.id.username = username.to_string().trim_ascii().to_string();
            server_sender.send( Message {
                contents: username.to_string(),
                owner: client_reference,
                message_id: rand::rng().next_u64() as usize,
                signal: Some(Username) } ).unwrap();
        } else {
            let message = Message {
                contents: message.clone(),
                owner: client_reference,
                message_id: rand::rng().next_u64() as usize,
                signal: Message::parse_signal(message.clone()) };
            server_sender.send(message).unwrap();
        }
    }
}

fn writer_socket(receiver: Receiver<Message>, mut stream: TcpStream) {
    loop {
        if let Ok(message) = receiver.recv() {
            match stream.write_all(message.contents.as_bytes()) { _ => {} }
        } else {
            break;
        }
    }
}

fn db_handler(db: Arc<Mutex<ServerDB>>, receiver: Receiver<Action>) {
    loop {
        if let Ok(action) = receiver.recv() {
            match action {
                UserDisconnect(_) => {
                    db.lock().unwrap().dec_user_count();
                },
                UserConnect(_) => {
                    db.lock().unwrap().inc_user_count();
                },
                _ => {

                }
            }
        }
    }
}

fn server_handler(server: Server, db_sender: Sender<Action>, senders: Arc<Mutex<Vec<Client>>>) {
    let server_id = ClientID { tag: Tag { tag: 0 }, username: "Server".to_string(), id: 0 };
    let server_client = Arc::new(Client { sender: mpsc::channel::<Message>().0, id: server_id });
    loop {
        if let Ok(mut message) = server.receiver.recv() {
            if message.contents.is_empty() && message.signal.is_none() {
                continue;
            }
            let mut clients = senders.lock().unwrap();
            match message.clone().signal {
                None => {
                    for client in clients.iter() {
                        if client.id == message.owner.id || message.contents.is_empty() {
                            continue;
                        }
                        message.contents = format!("{}#{}: {}", message.owner.id.username.to_string().trim_ascii(), message.owner.id.tag, message.contents.trim_ascii());
                        client.sender.send(message.clone()).unwrap();
                    }
                }
                Some(signal) => {
                    if signal == Disconnect {
                        println!("{:?} signal from {}", signal, message.owner.id);
                        db_sender.send(UserDisconnect(Arc::clone(&message.owner))).unwrap();
                        let owner = message.owner;
                        let mut idx: i32 = -1;
                        for client in clients.iter() {
                            if client.id == owner.id {
                                idx = clients.iter().position(|x| x.id == owner.id).unwrap() as i32;
                            } else {
                                client.sender.send(Message { contents: format!("User {} Disconnected", owner.id.username.to_string().trim_ascii()).to_string(),
                                    owner: Arc::clone(&owner), message_id: message.message_id, signal: None}).unwrap();
                            }
                        }
                        if idx != -1 {
                            clients.remove(idx as usize);
                        }
                    } else if signal == Connect {
                        println!("{:?} signal from {}", signal, message.owner.id);
                        db_sender.send(UserConnect(message.owner)).unwrap();
                    } else if signal == Username {
                        for client in clients.iter_mut() {
                            if client.id == message.owner.id {
                                client.id.username = message.clone().contents.trim_ascii().to_string();
                                println!("{:?} signal from {}", signal, client.id.username);
                            }
                        }
                    } else if signal == UserCommand {
                        match Message::parse_command(message.clone().contents).unwrap() {
                            PM(user) => {
                                let mut user_found = false;
                                for client in clients.iter() {
                                    if client.id.username_tag().eq(user.trim_ascii()) {
                                        user_found = true;
                                        let contents = format!("From {}: {}", message.owner.id.username_tag(), Message::parse_pm_message(message.clone().contents));
                                        let message = Message { contents, owner: message.owner.clone(), message_id: message.message_id, signal: None };
                                        client.sender.send(message).unwrap();
                                    }
                                }
                                if !user_found {
                                    message.owner.sender.send(Message {
                                        contents: String::from("That user does not exist."),
                                        owner: Arc::clone(&server_client),
                                        message_id: 0,
                                        signal: None,
                                    }).unwrap();
                                }
                            }
                            _ => {

                            }
                        }
                    }
                }
            }
        }
    }
}