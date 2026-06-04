use common::ClientPacket::{ConnectionRejected, Disconnect, InitialResponse};
use common::ServerEvent::{ChatMessageReceive, ConnectionAccept, ConnectionReject, Error, Message, PrivateMessage, Shutdown, UserDisconnected, UsernameCheck, UsernameResponse};
use common::{ClientPacket, LoginInfo, Server, ServerEvent, Session, User};
use rand::{Rng, rng};
use std::collections::HashMap;
use std::io::{stdin};
use std::net::IpAddr;
use std::sync::Arc;
use clavis::{EncryptedPacket, EncryptedReader, EncryptedStream, EncryptedWriter};
use tokio::io::{ ReadHalf, WriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{Semaphore, mpsc};

#[deny(clippy::unwrap_used)]
#[deny(clippy::expect_used)]
#[deny(clippy::panic)]
#[deny(unused_must_use)]
#[tokio::main]
async fn main() {
    let listener = match TcpListener::bind("127.0.0.1:8080").await {
        Ok(tl) => tl,
        Err(e) => {
            eprintln!("Internal server error binding listener: {}", e);
            return;
        }
    };

    let (server_sender, server_recv) = mpsc::channel::<ServerEvent>(Semaphore::MAX_PERMITS);

    let sender_reference = Arc::new(server_sender);

    let server_state = match handle_server_start(server_recv).await {
        Some(sv) => sv,
        None => {
            eprintln!("Internal server error initializing server.");
            return;
        }
    };

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
            let ip_src = socket.local_addr().unwrap().ip();
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

async fn handle_new_connection(mut stream: EncryptedStream<TcpStream>, server_sender: Arc<Sender<ServerEvent>>, ip_src: IpAddr) {
    let (sender, mut receiver) = mpsc::channel::<ServerEvent>(100);
    let sender_ref = Arc::new(sender);

    let packet: ClientPacket = match stream.read_packet().await {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "Internal server error reading stream during handshake: {}",
                e
            );
            return;
        }
    };

    match packet {
        ClientPacket::InitialRequest { username } => {
            if let Err(e) = server_sender
                .send(UsernameCheck {
                    username: username.clone(),
                    sender: Arc::clone(&sender_ref),
                })
                .await
            {
                eprintln!("Internal server error: {}", e);
            }
            while let Some(response) = receiver.recv().await {
                match response {
                    UsernameResponse { username, new_user } => {
                        if let Err(e) = stream.write_packet(&InitialResponse { username, new_user }).await {
                            eprintln!("Error in writing packet to stream: {}", e);
                            return;
                        }
                        break;
                    }
                    _ => {}
                }
            }
        },
        _ => {}
    }

    let packet: ClientPacket = match stream.read_packet().await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error reading packet: {}", e);
            return;
        }
    };

    let (reader_stream, mut writer_stream) = stream.split();


    match packet {
        ClientPacket::LoginRequestPacket { username, password } => {
            let login_info = LoginInfo {
                username: username.trim_ascii().to_string(),
                password,
            };
            if let Err(e) = server_sender
                .send(ServerEvent::ConnectionRequest {
                    login_details: login_info,
                    sender: Arc::clone(&sender_ref),
                    ip_src
                })
                .await
            {
                eprintln!("Internal server error sending info to server: {}", e);
            }
        }
        _ => {}
    }

    loop {
        if let Some(response) = receiver.recv().await {
            if let ConnectionAccept { user, .. } = response {
                tokio::spawn(async move {
                    reader_socket(reader_stream, Arc::clone(&server_sender), user).await;
                });

                tokio::spawn(async move {
                    writer_socket(receiver, writer_stream).await;
                });
            } else if let ConnectionReject { reason } = response {
                if let Err(e) = writer_stream.write_packet(&ConnectionRejected { reason }).await {
                    eprintln!("Error in writing packet to stream: {}", e);
                    continue;
                }
            }
            break;
        }
    }
}

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
        _ => Error {
            message: "Internal server error".to_string(),
        },
    }
}

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
        _ => Disconnect,
    }
}

async fn handle_server_start(server_recv: Receiver<ServerEvent>) -> Option<Server> {
    let server = Server::new(server_recv).await;

    Some(server)
}

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

async fn server_handler(mut server: Server) {
    let server_identity = Arc::new(User {
        username: "Server".to_string(),
        user_id: 0,
    });
    loop {
        if let Some(event) = server.receiver.recv().await {
            match event {
                ServerEvent::ConnectionRequest {
                    login_details,
                    sender,
                    ip_src,
                } => {
                    let session = Session::new(
                        Arc::clone(&sender),
                        ip_src,
                        rng().next_u64() as usize,
                        server.next_uid,
                    );
                    let session_arc = Arc::new(session);
                    println!(
                        "Connection request from: {}",
                        login_details.username.trim_ascii()
                    );
                    match server
                        .create_new_user(
                            login_details.username.clone(),
                            login_details.password.clone(),
                        )
                        .await
                    {
                        Some(user) => {
                            let user_arc = Arc::new(user);
                            server.user_id_map.insert(
                                user_arc.user_id,
                                (Arc::clone(&user_arc), Arc::clone(&session_arc)),
                            );
                            if let Err(e) = sender
                                .send(ConnectionAccept {
                                    user: Arc::clone(&user_arc),
                                    session: Arc::clone(&session_arc),
                                })
                                .await
                            {
                                eprintln!("Internal server error sending user info: {}", e);
                                continue;
                            }
                        }
                        None => {
                            if !server.verify_credentials(&login_details).await {
                                if let Err(e) = sender
                                    .send(ConnectionReject {
                                        reason: "Incorrect password".to_string(),
                                    })
                                    .await
                                {
                                    eprintln!("Internal server error sending message: {}", e);
                                }
                                continue;
                            }
                            let user = match server
                                .find_user_from_username(login_details.username)
                                .await
                            {
                                Some(u) => u,
                                None => {
                                    eprintln!("Unexpected error from querying db.");
                                    continue;
                                }
                            };
                            let user_arc = Arc::new(user);
                            server.user_id_map.insert(
                                user_arc.user_id,
                                (Arc::clone(&user_arc), Arc::clone(&session_arc)),
                            );
                            if let Err(e) = sender
                                .send(ConnectionAccept {
                                    user: Arc::clone(&user_arc),
                                    session: Arc::clone(&session_arc),
                                })
                                .await
                            {
                                eprintln!(
                                    "Internal server error in sending connection accept packet: {}",
                                    e
                                );
                                continue;
                            }
                        }
                    }
                }
                ChatMessageReceive { from, contents } => {
                    let uid_map = &server.user_id_map;
                    let user = match uid_map.get(&from) {
                        Some(user) => &user.0,
                        None => {
                            continue;
                        }
                    };
                    handle_broadcast(
                        contents,
                        user,
                        &server.user_id_map.values().collect::<Vec<_>>(),
                    )
                    .await;
                }
                UserDisconnected { user } => {
                    println!("{} has disconnected", user.username);
                    handle_client_disconnect(
                        &user,
                        Arc::clone(&server_identity),
                        &mut server.user_id_map,
                    )
                    .await;
                }
                PrivateMessage { to, from, contents } => {
                    let uid_map = &server.user_id_map;
                    let user_from = match server.user_id_map.get(&from) {
                        Some(u) => u,
                        None => {
                            continue;
                        }
                    };
                    if let Some(user_to) = server.find_user_from_username(to).await {
                        handle_pm(&user_to, &user_from.0, contents, uid_map).await;
                    }
                }
                UsernameCheck { username, sender } => {
                    let new_user;
                    println!("Checking user: {}", username);
                    match server.find_user_from_username(username.clone()).await {
                        Some(_) => {
                            new_user = false;
                        }
                        None => {
                            new_user = true;
                        }
                    }
                    if let Err(e) = sender.send(UsernameResponse { username, new_user }).await {
                        eprintln!("Internal server error sending username response: {}", e);
                    }
                }
                Shutdown => {
                    std::process::exit(0);
                }
                _ => {}
            }
        }
    }
}

async fn handle_pm(
    to: &User,
    from: &User,
    message: String,
    current_users: &HashMap<usize, (Arc<User>, Arc<Session>)>,
) {
    let formatted = format!("From {}: {}", from.username, message);
    if let Some((_, session)) = current_users.get(&to.user_id) {
        if let Err(e) = session
            .sender
            .send(Message {
                contents: formatted,
            })
            .await
        {
            eprintln!("Internal server error sending message: {}", e);
        }
    }
}
async fn handle_client_disconnect(
    user: &User,
    server_identity: Arc<User>,
    clients: &mut HashMap<usize, (Arc<User>, Arc<Session>)>,
) {
    if let None = clients.remove(&user.user_id) {
        eprintln!("Could not remove client from client list.");
    }
    handle_broadcast(
        format!("{} has disconnected.", user.username),
        &server_identity,
        &clients.values().collect::<Vec<_>>(),
    )
    .await;
}

async fn handle_broadcast(
    contents: String,
    originator: &User,
    clients: &Vec<&(Arc<User>, Arc<Session>)>,
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

    for (user, session) in clients {
        if originator.user_id == user.user_id {
            continue;
        } else {
            if let Err(e) = session
                .sender
                .send(Message {
                    contents: formatted.clone(),
                })
                .await
            {
                eprintln!("Internal server error sending broadcast to client: {}", e);
            }
        }
    }
}
