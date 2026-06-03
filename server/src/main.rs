use common::ClientPacket::{ConnectionRejected, Disconnect};
use common::ServerEvent::{Message, ChatMessageReceive, ConnectionAccept, ConnectionReject, Error, PrivateMessage, Shutdown, UserCreation};
use common::{ClientPacket, Session, ServerDB, ServerEvent, Server, LoginInfo, User, ConnectionResult};
use rand::{Rng, rng};
use std::collections::HashMap;
use std::fs::{OpenOptions};
use std::io::{BufWriter, Write, stdin};
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{Semaphore, mpsc};
use turso::Connection;
use common::ConnectionResult::{AcceptedCurrentUser, AcceptedNewUser, Rejected};

#[deny(clippy::unwrap_used)]
#[deny(clippy::expect_used)]
#[deny(clippy::panic)]
#[deny(unused_must_use)]
#[tokio::main]
async fn main() {
    let listener = match TcpListener::bind("127.0.0.1:8080").await {
        Ok(tl) => tl,
        Err(e) => {
            eprintln!("Error binding listener: {}", e);
            return;
        }
    };

    let (server_sender, server_recv) = mpsc::channel::<ServerEvent>(Semaphore::MAX_PERMITS);

    let sender_reference = Arc::new(server_sender);

    let server_state = match handle_server_start(server_recv).await {
        Some(sv) => sv,
        None => {
            eprintln!("Error initializing server.");
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
            tokio::spawn(async move {
                handle_new_connection(socket, sender).await;
            });
        }
    }
}

async fn handle_new_connection(stream: TcpStream, server_sender: Arc<Sender<ServerEvent>>) {
    let (sender, mut receiver) = mpsc::channel::<ServerEvent>(100);
    let mut stream = BufReader::new(stream);

    let mut line = String::new();

    if let Err(e) = stream.read_line(&mut line).await {
        eprintln!("Error reading stream during handshake: {}", e);
        return;
    }

    match serde_json::from_str::<ClientPacket>(&line) {
        Ok(ClientPacket::SessionRequest) => {
            if let Err(e) = stream.write("Enter username: \n".as_bytes()).await {
                eprintln!("Error writing username prompt: {}", e);
            }
        }
        _ => {}
    }

    line = String::new();

    let byte_val = match stream.read_line(&mut line).await {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Error reading stream: {}", e);
            return;
        }
    };

    let (reader_stream, mut writer_stream) = stream.into_inner().into_split();

    if byte_val == 0 {
        println!("Client disconnected during handshake.");
        return;
    }

    let packet: ClientPacket = match serde_json::from_str(&line) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error parsing packet: {}", e);
            return;
        }
    };

    let sender_ref = Arc::new(sender);
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
                    ip_src: reader_stream.local_addr().unwrap().ip()
                })
                .await
            {
                eprintln!("Error sending info to server: {}", e);
            }
        }
        _ => {}
    }

    loop {
        if let Some(response) = receiver.recv().await {
            if let ConnectionAccept { user, session } = response {

                tokio::spawn(async move {
                    reader_socket(
                        reader_stream,
                        Arc::clone(&server_sender),
                        user
                    )
                    .await;
                });

                tokio::spawn(async move {
                    writer_socket(receiver, writer_stream).await;
                });
            } else if let UserCreation {username, user_id} = response {

            }
            else if let ConnectionReject { reason } = response {
                if let Ok(serialized) = serde_json::to_string(&ConnectionRejected { reason }) {
                    let _ = writer_stream.write_all(serialized.as_bytes()).await;
                    let _ = writer_stream.write_all(b"\n").await;
                }
            }
            break;
        }
    }
}

async fn reader_socket(
    stream: OwnedReadHalf,
    server_sender: Arc<Sender<ServerEvent>>,
    user: User,
) {
    let mut stream = BufReader::new(stream);
    loop {
        let mut line = String::new();

        let byte_val = match stream.read_line(&mut line).await {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Error reading stream: {}", e);
                continue;
            }
        };

        if byte_val == 0 {
            if let Err(e) = server_sender
                .send(ServerEvent::UserDisconnected { user })
                .await
            {
                eprintln!("Error sending disconnect packet to server: {}", e);
            }
            break;
        }

        let packet: ClientPacket = match serde_json::from_str(&line) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Error parsing string to packet: {}", e);
                continue;
            }
        };

        let event = packet_to_event(packet, &user);

        if let Err(e) = server_sender.send(event).await {
            eprintln!("Error sending event to server: {}", e);
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
            message: "Error".to_string(),
        },
    }
}

async fn writer_socket(mut receiver: Receiver<ServerEvent>, mut stream: OwnedWriteHalf) {
    loop {
        if let Some(event) = receiver.recv().await {
            if let Ok(contents) = serde_json::to_string(&event_to_packet(event)) {
                let _ = stream.write_all(contents.as_bytes()).await;
                let _ = stream.write_all(b"\n").await;
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

async fn handle_server_shutdown(server_db: &ServerDB) {
    println!("Shutting down server.");

    let as_json = match serde_json::to_string(&server_db.login_info_vec) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error parsing: {}", e);
            return;
        }
    };

    let file = match OpenOptions::new().write(true).open("./users.json") {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error opening users.json: {}", e);
            return;
        }
    };

    let mut writer = BufWriter::new(file);

    if let Err(e) = writer.write_all(as_json.as_bytes()) {
        eprintln!("Error writing to users.json: {}", e);
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
            eprintln!("Error getting login_details input: {}", e);
            continue;
        }

        match cmd.trim_ascii() {
            "/shutdown" => {
                if let Err(e) = server_sender.send(Shutdown).await {
                    eprintln!("Error sending shutdown signal to server: {}", e);
                    continue;
                }
                break;
            }
            _ => {}
        }
    }
}

async fn server_handler(mut server: Server) {
    let server_identity = Arc::new(User { username: "Server".to_string(), user_id: 0 });
    loop {
        if let Some(event) = server.receiver.recv().await {
            match event {
                ServerEvent::ConnectionRequest { login_details, sender, ip_src } => {
                    println!("Connection request from: {}", login_details.username.trim_ascii());
                    let is_accepted = handle_client_connect(
                        login_details,
                        &mut server.db_conn
                    )
                        .await;
                    let session = Session::new(Arc::clone(&sender), ip_src, rng().next_u64() as usize, server.next_uid);
                    match is_accepted {
                        AcceptedCurrentUser { uid } => {
                            let user = match server.load_user(uid).await {
                                Some(u) => u,
                                None => {
                                    eprintln!("User does not exist.");
                                    continue;
                                }
                            };
                            println!("{user:?}");
                        },
                        AcceptedNewUser {username, password} => {
                            server.add_new_user(username.clone(), password).await;
                            if let Err(e) = sender.send(UserCreation {username: username.clone(), user_id: server.next_uid}).await {
                                eprintln!("Error sending user info: {}", e);
                            }
                            // server.add_new_session(user, session);
                        },
                        ConnectionResult::Rejected {reason} => {

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
                    handle_broadcast(contents, user, &server.user_id_map.values().collect::<Vec<_>>())
                        .await;
                }
                ServerEvent::UserDisconnected { user } => {
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

                    match uid_map.get(&from) {
                        Some(client) => {
                            handle_pm(&client.0, &client.0, contents, uid_map).await;
                        }
                        None => {
                            if let Some(client) = server.user_id_map.get(&0) {
                                if let Err(e) = client
                                    .1.sender
                                    .send(Error {
                                        message: String::from("User not found"),
                                    })
                                    .await
                                {
                                    eprintln!("Error sending message: {}", e);
                                }
                            }
                        }
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
    current_users: &HashMap<usize, (User, Session)>,
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
            eprintln!("Error sending message: {}", e);
        }
    }
}

fn create_new_user(
    username: String,
    uid_to_assign: usize,
) -> Option<User> {

    None
}

async fn handle_client_connect(
    login_details: LoginInfo,
    clients: &mut Connection,
) -> ConnectionResult {
    AcceptedNewUser {username: login_details.username, password: login_details.password}
}
async fn handle_client_disconnect(
    user: &User,
    server_identity: Arc<User>,
    clients: &mut HashMap<usize, (User, Session)>,
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
    clients: &Vec<&(User, Session)>,
) {
    if contents.is_empty() {
        return;
    }

    let formatted;

    if originator.user_id != 0 {
        formatted = format!(
            "{}: {}",
            originator.username,
            contents.trim_ascii()
        );
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
                eprintln!("Error sending broadcast to client: {}", e);
            }
        }
    }
}
