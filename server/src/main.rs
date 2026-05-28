use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Arc};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use rand::{rng, Rng};
use common::{Client, ClientID, Message, Packet, ServerDB, ServerState, Tag};
use common::ServerEvent;
use common::ServerEvent::*;
use std::collections::HashMap;
use std::fs::read;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:8080").unwrap();

    let (db_sender, db_recv) = mpsc::channel::<ServerEvent>();
    let (server_sender, server_recv) = mpsc::channel::<ServerEvent>();

    let db = ServerDB { message_history: Vec::new(), receiver: db_recv };

    let server_state = ServerState::new(db_sender, server_recv);

    thread::spawn(move || {
       server_handler(server_state);
    });

    let sender_reference = Arc::new(server_sender);

    while let Ok((stream, _)) = listener.accept() {
        let sender = Arc::clone(&sender_reference);
        thread::spawn(move || {
            handle_new_connection(stream, Arc::clone(&sender));
        });
    }
}

fn handle_new_connection(mut stream: TcpStream, server_sender: Arc<Sender<ServerEvent>>) {
    let (sender, receiver) = mpsc::channel::<ServerEvent>();

    let sender_ref = Arc::new(sender.clone());

    let mut str_buffer = String::new();

    let reader_stream = stream.try_clone().unwrap();
    let writer_stream = stream.try_clone().unwrap();

    let mut reader = BufReader::new(stream);

    let byte_val = reader.read_line(&mut str_buffer).unwrap();

    if byte_val == 0 {
        return;
    }

    match &str_buffer[..].split(" ").collect::<Vec<_>>()[..] {
        ["USERNAME", username] => {
            server_sender.send(IdentityRequest(username.to_string(), sender_ref)).unwrap();
        }
        _ => {

        }
    }

    loop {
        if let Ok(client_info) = receiver.recv() {
            if let IdentityAssignment(mut id) = client_info {

                id.username = id.username.split_terminator("\n").collect::<String>();

                let client = Client { sender, id: id.clone() };
                server_sender.send(Connect(client)).unwrap();

                thread::spawn(move || {
                    reader_socket(reader_stream, Arc::clone(&server_sender), Arc::new(id));
                });

                thread::spawn(move || {
                    writer_socket(receiver, writer_stream);
                });
                break;
            }
        }
    }
}

fn reader_socket(stream: TcpStream, server_sender: Arc<Sender<ServerEvent>>, client: Arc<ClientID>) {
    let mut reader = BufReader::new(stream);
    let mut str_buf = String::new();
    loop {
        let byte_value = reader.read_line(&mut str_buf).unwrap();

        if byte_value == 0 {
            server_sender.send(Disconnect(client)).unwrap();
            break;
        }

        let event_type = str_buf.split(" ").next().unwrap();

        match event_type {
            "CHAT" => {
                let contents = str_buf.clone();

                server_sender.send(ChatMessage(Message { contents:  str_buf.clone(), owner: Arc::clone(&client), message_id: 0 })).unwrap();
            },
            _ => {

            }
        }

    }
}



fn writer_socket(receiver: Receiver<ServerEvent>, mut stream: TcpStream) {
    loop {
        if let Ok(ChatMessage(message)) = receiver.recv() {
            match stream.write_all(message.contents.as_bytes()) { _ => {} }
        } else {
            break;
        }
    }
}

fn server_handler(mut server: ServerState) {
    loop {
        if let Ok(event) = server.receiver.recv() {
            match event {
                ChatMessage(msg) => {
                    handle_broadcast(msg, &server.clients.values().collect::<Vec<_>>());
                },
                Disconnect(client) => {
                    handle_client_disconnect(&client, &mut server.clients);
                    println!("Disconnected {}", client);
                    handle_broadcast(Message { contents: String::from(format!("{}#{} has disconnected", client.username, client.tag)), owner: Arc::clone(&server.identity), message_id: 0 },
                                     &server.clients.values().collect());
                },
                Connect(client) => {
                    println!("New connection from {}", client.id);
                    handle_client_connect(client.id.clone(), client, &mut server.clients);
                },
                IdentityRequest(username, sender) => {
                    let id = assign_identity(username.clone(), &server.clients);
                    println!("Assigned new identity to user with ID {}", id.id);
                    sender.send(IdentityAssignment(id)).unwrap();
                }
                _ => {

                }
            }
        }
    }
}


fn assign_identity(username: String, current_ids: &HashMap<ClientID, Client>) -> ClientID {
    let tag = Tag::new();
    let id = ClientID { username: username.clone(), tag, id: rng().next_u64() as usize };

    if current_ids.contains_key(&id) {
        return assign_identity(username, current_ids);
    }

    id
}

fn handle_client_connect(client_id: ClientID, client: Client, clients: &mut HashMap<ClientID, Client>) {
    clients.insert(client_id, client);
}
fn handle_client_disconnect(client: &ClientID, clients: &mut HashMap<ClientID, Client>) {
    clients.remove(&client).unwrap();
}

fn handle_broadcast(message: Message, clients: &Vec<&Client>) {
    if message.contents.is_empty() {
        return;
    }
    let originator = message.clone().owner;

    for client in clients {
        if client.id == *originator {
            continue;
        } else {
            client.sender.send(ChatMessage(message.clone())).unwrap();
        }
    }
}
