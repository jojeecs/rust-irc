//! # Chat Server
//!
//! The main server application for the chat system. It handles client connections,
//! authentication, and message broadcasting.

use common::ClientPacket::{Disconnect};
use common::ServerEvent::{ChatMessageReceive, AuthenticationAccept, AuthenticationReject, LoginRequest, Error, Message, PrivateMessage, Shutdown, UserDisconnected, DirectMessageExternal};
use common::{ClientPacket, LoginInfo, Server, ServerEvent, Session, User};
use rand::{Rng, rng};
use std::collections::HashMap;
use std::io::{stdin};
use std::net::IpAddr;
use std::sync::Arc;
use clavis::{EncryptedPacket, EncryptedReader, EncryptedStream, EncryptedWriter};
use cliclack::input;
use tokio::io::{ReadHalf, WriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{Semaphore, mpsc};
use regex::Regex;
use common::room::room::Room;

#[deny(clippy::unwrap_used)]
#[deny(clippy::expect_used)]
#[deny(clippy::panic)]
#[deny(unused_must_use)]
/// Starts the server and begins listening for connections.
#[tokio::main]
async fn main() {
    let ip_regex = match Regex::new(r"^(?:[0-9]{1,3}\.){3}[0-9]{1,3}$") {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Invalid regex: {}", e);
            return;
        }
    };
    let ip: String = match input("Enter IP to listen on")
        .placeholder("127.0.0.1")
        .default_input("127.0.0.1")
        .validate(move |input: &String| {
            if ip_regex.clone().is_match(input.trim()) {
                Ok(())
            } else {
                Err("Invalid IP")
            }
        })
        .interact()
    {
        Ok(s) => s,
        Err(_) => {
            return;
        }
    };
    let listener = match TcpListener::bind(format!("{}:8080", ip)).await {
        Ok(tl) => tl,
        Err(e) => {
            eprintln!("Internal server error binding listener: {}", e);
            return;
        }
    };

    println!("Listening on {}:8080", ip);

    let (server_sender, server_recv) = mpsc::channel::<ServerEvent>(Semaphore::MAX_PERMITS);

    let sender_reference = Arc::new(server_sender);

    let mut server_state = match handle_server_start(server_recv).await {
        Some(sv) => sv,
        None => {
            eprintln!("Internal server error initializing server.");
            return;
        }
    };

    server_state.room_store.add_room_new("Global".to_string());
    server_state.room_store.add_room_new("Private".to_string());

    let sender_ref = Arc::clone(&sender_reference);

    tokio::spawn(async move {
        server_input_handler(sender_ref).await;
    });

    tokio::spawn(async move {
        server_handler(server_state).await;
    });

    loop {
        if let Ok((socket, _)) = listener.accept().await {
            let sender = Arc::clone(&sender_reference);
            let ip_src = match socket.local_addr() {
                Ok(s) => s.ip(),
                Err(e) => {
                    eprintln!("Error retrieving socket IP address: {}", e);
                    continue;
                }
            };
            let encrypted_socket = match EncryptedStream::new(socket, None).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Error encrypting socket: {}", e);
                    continue;
                }
            };
            tokio::spawn(async move {
                handle_new_connection(encrypted_socket, sender, ip_src).await;
            });
        }
    }
}

/// Handles a newly established client connection.
/// 
/// This function performs the initial authentication handshake and then
/// spawns reader and writer tasks for the connection.
async fn handle_new_connection(stream: EncryptedStream<TcpStream>, server_sender: Arc<Sender<ServerEvent>>, ip_src: IpAddr) {
    let (sender, mut receiver) = mpsc::channel::<ServerEvent>(100);
    let sender_ref = Arc::new(sender);
    let (mut reader_stream, mut writer_stream) = stream.split();

    println!("Connection received from: {}", ip_src);

    let user_found: Arc<User>;

    loop {
        if let Ok(packet) = reader_stream.read_packet::<ClientPacket>().await {
            match packet {
                ClientPacket::LoginRequestPacket {username, password} => {
                    println!("Login packet received.");
                    let login_details = LoginInfo { username, password };
                    if let Err(e) = server_sender.send(LoginRequest {login_details, sender: Arc::clone(&sender_ref), ip_src}).await {
                        eprintln!("Error: {}", e);
                    }

                    if let Some(response) = receiver.recv().await {
                        match response {
                            AuthenticationAccept {user, new_user, ..} => {
                                println!("Login accepted");
                                user_found = user;
                                let _ = writer_stream.write_packet(&ClientPacket::AuthenticationAccepted { new_user }).await;
                                break;
                            } AuthenticationReject {..} => {
                                let _ = writer_stream.write_packet(&ClientPacket::AuthenticationRejected).await;
                                println!("Login rejected");
                                continue;
                            }
                            _ => { continue; }
                        }
                    }
                }
                _ => { continue; }
            }
        } else {
            println!("Client disconnected while logging in, IP: {}", ip_src);
            return;
        }
    }


    tokio::spawn(async move {
        reader_socket(reader_stream, server_sender, user_found).await
    });

    tokio::spawn(async move {
        writer_socket(receiver, writer_stream).await
    }).await.unwrap();
}

/// Task responsible for reading packets from a client and converting them to server events.
async fn reader_socket(
    mut stream: EncryptedReader<ReadHalf<TcpStream>>,
    server_sender: Arc<Sender<ServerEvent>>,
    user: Arc<User>,
) {
    loop {
        let packet: ClientPacket = match stream.read_packet().await {
            Ok(p) => p,
            Err(e) => {
                if e.is_stream_error() {
                    if let Err(e) = server_sender.send(UserDisconnected {user}).await {
                        eprintln!("Internal server error sending disconnect message: {}", e);
                    }
                    break;
                }
                continue;
            }
        };


        let event = packet_to_event(packet, &user);

        if let Err(e) = server_sender.send(event).await {
            eprintln!("Internal server error sending event to server: {}", e);
        }
    }
}

/// Converts a `ClientPacket` into a `ServerEvent`.
fn packet_to_event(packet: ClientPacket, user: &User) -> ServerEvent {
    match packet {
        ClientPacket::PublicMessage { contents } => ChatMessageReceive {
            from: user.user_id,
            contents,
        },
        ClientPacket::PrivateMessage { to, contents, .. } => PrivateMessage {
            to,
            from: user.user_id,
            contents,
        },
        ClientPacket::RoomChange {new_room_name, old_room_name} => {
            ServerEvent::RoomChange {new_room_name, old_room_name, user_id: user.user_id}
        },
        _ => Error {
            message: "Internal server error".to_string(),
        },
    }
}

/// Task responsible for writing packets back to a client.
async fn writer_socket(mut receiver: Receiver<ServerEvent>, mut stream: EncryptedWriter<WriteHalf<TcpStream>>) {
    loop {
        if let Some(event) = receiver.recv().await {
            let packet = event_to_packet(event);
            if let Err(e) = stream.write_packet(&packet).await {
                eprintln!("Error writing packet to stream: {}", e);
                continue;
            }
        }
    }
}
fn event_to_packet(server_event: ServerEvent) -> ClientPacket {
    match server_event {
        Message { contents } => ClientPacket::PublicMessage { contents },
        DirectMessageExternal {to, contents, ..} => {
            ClientPacket::PrivateMessage {to, contents}
        }
        _ => Disconnect,
    }
}

async fn handle_server_start(server_recv: Receiver<ServerEvent>) -> Option<Server> {
    let server = Server::new(server_recv).await;

    Some(server)
}

/// Handles server-side console input for administrative commands.
async fn server_input_handler(server_sender: Arc<Sender<ServerEvent>>) {
    loop {
        let mut cmd = String::new();

        if let Err(e) = stdin().read_line(&mut cmd) {
            eprintln!("Internal server error getting login_details input: {}", e);
            continue;
        }

        match cmd.trim_ascii() {
            "/shutdown" => {
                if let Err(e) = server_sender.send(Shutdown).await {
                    eprintln!(
                        "Internal server error sending shutdown signal to server: {}",
                        e
                    );
                    continue;
                }
                break;
            }
            _ => {}
        }
    }
}

/// The main event loop of the server, processing all `ServerEvent`s.
async fn server_handler(mut server: Server) {
    let mut server_identity = Arc::new(User {
        username: "Server".to_string(),
        user_id: 0,
    });
    loop {
        if let Some(event) = server.receiver.recv().await {
            match event {
                LoginRequest {login_details, sender, ip_src} => {
                    let user;
                    let mut user_id = 0;
                    if server.user_exists(&login_details.username).await {
                        user = match server.find_user_from_username(login_details.username).await {
                            Some(u) => {
                                user_id = u.user_id;
                                Arc::new(u)
                            },
                            None => {
                                eprintln!("Internal server error fetching user; this should not have happened");
                                continue;
                            }
                        };
                    } else {
                        user = match server.create_new_user(login_details.username, login_details.password).await {
                            Some(u) => Arc::new(u),
                            None => {
                                println!("Error creating new user");
                                continue;
                            }
                        };
                        user_id = user.user_id;
                    }

                    let session = Session::new(Arc::clone(&sender), ip_src, rng().next_u64() as usize, user_id);

                    let session_ref = Arc::new(session);

                    server.user_id_map.insert(user_id, (Arc::clone(&user), Arc::clone(&session_ref)));
                    server.username_map.insert(user.username.clone(), user.user_id);
                    if let Some(room) = server.room_store.get_room_from_name(&"Global".to_string()) {
                        room.add_session(Arc::clone(&session_ref));
                        let _ = server.user_room_map.insert(user_id, "Global".to_string());
                    }

                    let _ = sender.send(AuthenticationAccept {user: Arc::clone(&user), new_user: false }).await;
                }
                ChatMessageReceive { from, contents } => {
                    let uid_map = &server.user_id_map;
                    let user = match uid_map.get(&from) {
                        Some(user) => &user.0,
                        None => {
                            continue;
                        }
                    };

                    let room_name = match server.user_room_map.get(&user.user_id) {
                        Some(r) => r,
                        None => {
                            continue;
                        }
                    };

                    let room = match server.room_store.get_room_from_name(room_name) {
                        Some(r) => r,
                        None => {
                            continue;
                        }
                    };

                    handle_broadcast(
                        contents,
                        user,
                        room,
                    )
                    .await;
                }
                UserDisconnected { user } => {
                    println!("{} has disconnected", user.username);

                    let room_name = match server.user_room_map.get(&user.user_id) {
                        Some(r) => r,
                        None => {
                            continue;
                        }
                    };

                    let session = match &server.get_session_from_uid(user.user_id).await {
                        Some(s) => s,
                        None => {
                            continue;
                        }
                    }.clone();

                    let room = match server.room_store.get_room_from_name(room_name) {
                        Some(r) => r,
                        None => {
                            continue;
                        }
                    };

                    handle_client_disconnect(
                        &user,
                        session,
                        Arc::clone(&server_identity),
                        room,
                    )
                    .await;
                },
                PrivateMessage { to, from, contents } => {
                    println!("Sending private message");
                    let user_from = match server.user_id_map.get(&from) {
                        Some(u) => u,
                        None => {
                            continue;
                        }
                    }.clone().0;

                    let to_session = match server.get_session_from_username(to.clone()).await {
                        Some(s) => s,
                        None => {
                            println!("Unable to find session to send to.");
                            continue;
                        }
                    }.clone();

                    let from_session = match server.user_id_map.get(&from) {
                        Some((_, s)) => s,
                        None => {
                            println!("Unable to find session to send from.");
                            continue;
                        }
                    }.clone();

                    handle_pm(to, &user_from, contents, to_session, from_session).await;
                },
                ServerEvent::RoomChange {new_room_name, old_room_name, user_id} => {
                    let session = match server.user_id_map.get(&user_id) {
                        Some(u) => {
                            u
                        },
                        None => {
                            eprintln!("Error looking up user info to change rooms.");
                            continue;
                        }
                    }.clone();

                    let old_room = match server.room_store.get_room_from_name(&old_room_name) {
                        Some(r) => r,
                        None => {
                            eprintln!("Error fetching old room {old_room_name}");
                            continue;
                        }
                    };

                    if let None = old_room.remove_session(Arc::clone(&session.1)) {
                        println!("Error removing session from old room {old_room_name}");
                        continue;
                    }

                    let new_room = match server.room_store.get_room_from_name(&new_room_name) {
                        Some(r) => r,
                        None => {
                            eprintln!("Error fetching new room {new_room_name}");
                            continue;
                        }
                    };


                    let _ = server.user_room_map.insert(user_id, new_room_name);


                    new_room.add_session(session.1);
                }
                Shutdown => {
                    std::process::exit(0);
                }
                _ => {}
            }
        }
    }
}

/// Sends a private message from one user to another.
async fn handle_pm(
    to: String,
    from: &User,
    message: String,
    to_session: Arc<Session>,
    from_session: Arc<Session>,
) {
    let from_formatted = format!("From {}: {}", from.username, message);
    let to_formatted = format!("To {}: {}", to, message);

    let _ = to_session.sender.send(DirectMessageExternal {to: to.clone(), from: from.username.clone(), contents: from_formatted}).await;
    let _ = from_session.sender.send(DirectMessageExternal {to, from: from.username.clone(), contents: to_formatted}).await;
}
async fn handle_client_disconnect(
    user: &User,
    session: Arc<Session>,
    server_identity: Arc<User>,
    room: &mut Room,
) {
    if let None = room.remove_session(session) {
        eprintln!("Could not remove client from client list.");
    }
    handle_broadcast(
        format!("{} has disconnected.", user.username),
        &server_identity,
        room,
    )
    .await;
}

/// Broadcasts a message to all connected clients.
async fn handle_broadcast(
    contents: String,
    originator: &User,
    room: &mut Room,
) {
    if contents.is_empty() {
        return;
    }

    let formatted;

    if originator.user_id != 0 {
        formatted = format!("{}: {}", originator.username, contents.trim_ascii());
    } else {
        formatted = format!("SERVER: {}", contents.trim_ascii());
    }

    room.new_message(formatted).await;
}
