use common::ClientPacket::{Disconnect, HTTPRequest};
use common::ServerEvent::{
    Broadcast, ChatMessageReceive, ConnectionAccepted, ConnectionRejected, Error, HTTPResponse,
    PrivateMessage,
};
use common::UserPrivilege::Member;
use common::{Client, ClientID, ClientPacket, ClientSession, ServerEvent, ServerState};
use rand::{rng, Rng, RngExt};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc};
use std::{thread};

fn main() {
    let listener = TcpListener::bind("127.0.0.1:8080").unwrap();

    let (server_sender, server_recv) = mpsc::channel::<ServerEvent>();

    let server_state = ServerState::new(server_recv);

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


fn handle_new_connection(stream: TcpStream, server_sender: Arc<Sender<ServerEvent>>) {
    let (sender, receiver) = mpsc::channel::<ServerEvent>();

    let mut line = String::new();

    let reader_stream = stream.try_clone().unwrap();
    let mut writer_stream = stream.try_clone().unwrap();

    let mut reader = BufReader::new(stream);

    let byte_val = reader.read_line(&mut line).unwrap();

    if byte_val == 0 {
        println!(
            "Error during handshake. Connection information: IP: {}",
            writer_stream.peer_addr().unwrap().ip()
        );
        return;
    }

    let packet: ClientPacket = serde_json::from_str(&line).unwrap_or_else(|_| {
        match &line.lines().next().unwrap().split(" ").collect::<Vec<_>>()[..] {
            ["GET", resource, "HTTP/1.1"] => HTTPRequest {
                resource: resource.to_string(),
            },
            _ => HTTPRequest {
                resource: String::from("/"),
            },
        }
    });

    match packet {
        ClientPacket::Connect { username } => {
            server_sender
                .send(ServerEvent::ConnectionRequest {
                    username: username.trim_ascii().to_string(),
                    sender,
                })
                .unwrap();
        }
        HTTPRequest { resource } => {
            let sender = Arc::new(sender);
            let mut resource = resource.strip_prefix("/").unwrap().to_string();
            if resource.is_empty() {
                resource = String::from("index.html");
            }
            server_sender
                .send(ServerEvent::HTTPRequest {
                    resource: resource.clone(),
                    sender: Arc::clone(&sender),
                })
                .unwrap();
        }
        _ => {}
    }

    loop {
        if let Ok(response) = receiver.recv() {
            if let ConnectionAccepted { client_id } = response {
                let client = Client {
                    client_id,
                    privilege: Member,
                };

                thread::spawn(move || {
                    reader_socket(reader_stream, Arc::clone(&server_sender), Arc::new(client));
                });

                thread::spawn(move || {
                    writer_socket(receiver, writer_stream);
                });
            } else if let HTTPResponse {
                status,
                contents,
                ..
            } = response
            {
                let response = format!("{status}\r\nLocation: {contents}\r\n\r\nContent-Type: text/plain; charset=utf-8");
                writer_stream.write_all(response.as_bytes()).unwrap();
            }
            break;
        }
    }
}

fn reader_socket(stream: TcpStream, server_sender: Arc<Sender<ServerEvent>>, client: Arc<Client>) {
    let mut reader = BufReader::new(stream);
    loop {
        let mut line = String::new();

        let byte_val = reader.read_line(&mut line).unwrap();

        if byte_val == 0 {
            server_sender
                .send(ServerEvent::UserDisconnected { user: client })
                .unwrap();
            break;
        }

        let packet: ClientPacket = serde_json::from_str(&line).unwrap();

        let event = packet_to_event(packet, Arc::clone(&client));

        server_sender.send(event).unwrap();
    }
}

fn packet_to_event(packet: ClientPacket, client: Arc<Client>) -> ServerEvent {
    match packet {
        ClientPacket::ChatMessage { contents } => ChatMessageReceive {
            from: Arc::clone(&client),
            contents,
        },
        ClientPacket::PrivateMessage { to, contents, .. } => PrivateMessage {
            to,
            from: Arc::clone(&client),
            contents,
        },
        _ => Error {
            message: "Error".to_string(),
        },
    }
}

fn writer_socket(receiver: Receiver<ServerEvent>, mut stream: TcpStream) {
    loop {
        if let Ok(event) = receiver.recv() {
            stream
                .write_all(
                    serde_json::to_string(&event_to_packet(event))
                        .unwrap()
                        .as_bytes(),
                )
                .unwrap();
            stream.write_all(b"\n").unwrap();
        }
    }
}

fn event_to_packet(server_event: ServerEvent) -> ClientPacket {
    match server_event {
        Broadcast { contents } => ClientPacket::ChatMessage { contents },
        ServerEvent::Message { contents } => {
            ClientPacket::ChatMessage { contents  }
        }
        _ => Disconnect,
    }
}

fn server_handler(mut server: ServerState) {
    loop {
        if let Ok(event) = server.receiver.recv() {
            match event {
                ServerEvent::ConnectionRequest { username, sender } => {
                    println!("Connection request from: {username}");
                    handle_client_connect(username, sender, &mut server.clients);
                }
                ChatMessageReceive { from, contents } => {
                    println!(
                        "Broadcasting message \"{}\" from {}#{} ",
                        contents, from.client_id.username, from.client_id.chat_tag
                    );
                    handle_broadcast(contents, from, &server.clients.values().collect::<Vec<_>>());
                }
                ServerEvent::UserDisconnected { user } => {
                    handle_client_disconnect(user, &mut server.clients);
                }
                PrivateMessage { to, from, contents } => {
                    let username = to
                        .split("#")
                        .collect::<Vec<_>>()
                        .get(0)
                        .unwrap()
                        .to_string();
                    let tag = to
                        .split("#")
                        .collect::<Vec<_>>()
                        .get(1)
                        .unwrap()
                        .to_string()
                        .parse::<usize>()
                        .unwrap();

                    match find_user(&username, tag, &server.clients) {
                        Some(client) => {
                            handle_pm(&client, from, contents, &server.clients);
                        }
                        None => {
                            server
                                .clients
                                .get(&from)
                                .unwrap()
                                .sender
                                .send(Error {
                                    message: String::from("User not found"),
                                })
                                .unwrap();
                        }
                    }
                }
                ServerEvent::HTTPRequest {  sender, .. } => {
                    let contents =
                        "https://jojeecs.github.io/portfolio/".to_string();
                    let length = contents.len();
                    let status = String::from("HTTP/1.1 307 Temporary Redirect");

                    sender
                        .send(HTTPResponse {
                            status,
                            contents,
                            length,
                        })
                        .unwrap();
                }
                _ => {}
            }
        }
    }
}

fn find_user<'a>(
    username: &String,
    tag: usize,
    clients: &'a HashMap<Client, ClientSession>,
) -> Option<&'a Client> {
    for client in clients.keys().collect::<Vec<_>>() {
        if client.client_id.username.eq(username) && client.client_id.chat_tag.eq(&tag) {
            return Some(&client);
        }
    }

    None
}

fn handle_pm(
    to: &Client,
    from: Arc<Client>,
    message: String,
    current_users: &HashMap<Client, ClientSession>,
) {
    let formatted = format!(
        "{}#{}: {}",
        from.client_id.username, from.client_id.chat_tag, message
    );
    current_users
        .get(to)
        .unwrap()
        .sender
        .send(ServerEvent::Message {
            contents: formatted,
        })
        .unwrap();
}

fn assign_identity(
    username: String,
    current_ids: &HashMap<Client, ClientSession>,
) -> Option<Client> {
    let user_id = ClientID {
        username: username.trim_ascii().to_string(),
        chat_tag: rng().random_range(1000..10000),
        id: rng().next_u64() as usize,
    };
    let id = Client {
        client_id: user_id,
        privilege: Member,
    };

    if current_ids.contains_key(&id) {
        return assign_identity(username, current_ids);
    }

    Some(id)
}

fn handle_client_connect(
    username: String,
    sender: Sender<ServerEvent>,
    clients: &mut HashMap<Client, ClientSession>,
) {
    let new_client = assign_identity(username, &clients);
    if new_client.is_some() {
        let client = new_client.unwrap();
        let id = client.client_id.clone();
        sender
            .send(ConnectionAccepted {
                client_id: id.clone(),
            })
            .unwrap();
        clients.insert(
            client,
            ClientSession {
                client_id: id,
                sender,
            },
        );
    } else {
        sender
            .send(ConnectionRejected {
                reason: "Incorrect username formatting".to_string(),
            })
            .unwrap();
    }
}
fn handle_client_disconnect(client: Arc<Client>, clients: &mut HashMap<Client, ClientSession>) {
    handle_broadcast(
        format!(
            "{}#{} has left the chat.",
            client.client_id.username, client.client_id.chat_tag
        )
        .to_string(),
        Arc::clone(&client),
        &clients.values().collect::<Vec<_>>(),
    );
    clients.remove(&client).unwrap();
}

fn handle_broadcast(contents: String, originator: Arc<Client>, clients: &Vec<&ClientSession>) {
    if contents.is_empty() {
        return;
    }

    let formatted = format!(
        "{}#{}: {}",
        originator.client_id.username, originator.client_id.chat_tag, contents
    );

    for session in clients {
        if session.client_id.id == originator.client_id.id {
            continue;
        } else {
            session
                .sender
                .send(Broadcast {
                    contents: formatted.clone(),
                })
                .unwrap();
        }
    }
}
