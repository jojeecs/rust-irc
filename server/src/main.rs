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

#[deny(clippy::unwrap_used)]
#[deny(clippy::expect_used)]
#[deny(clippy::panic)]
#[deny(unused_must_use)]
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
                                continue;
                            }
                        }
                    }

                    let session = Session::new(Arc::clone(&sender), ip_src, rng().next_u64() as usize, user_id);

                    let session_ref = Arc::new(session);

                    server.user_id_map.insert(user_id, (Arc::clone(&user), Arc::clone(&session_ref)));

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
    let from_formatted = format!("From {}: {}", from.username, message);
    let to_formatted = format!("To {}: {}", to.username, message);
    let from_session = match current_users.get(&from.user_id) {
        Some((_, s)) => s,
        None => {
            return;
        }
    };
    let to_session = match current_users.get(&to.user_id) {
        Some((_, s)) => s,
        None => {
            let _ = from_session.sender.send(Error {message: format!("User {} not found or not online.", to.username)}).await;
            return;
        }
    };
    let _ = to_session.sender.send(DirectMessageExternal {to: to.username.clone(), from: from.username.clone(), contents: from_formatted}).await;
    let _ = from_session.sender.send(DirectMessageExternal {to: to.username.clone(), from: from.username.clone(), contents: to_formatted}).await;
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

    for (_, session) in clients {
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
