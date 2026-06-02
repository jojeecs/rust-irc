use std::ascii::AsciiExt;
use common::ClientPacket::{ConnectionReject, Disconnect, HTTPRequest};
use common::ServerEvent::{Broadcast, ChatMessageReceive, ConnectionAccepted, ConnectionRejected, Error, HTTPResponse, PrivateMessage, Shutdown};
use common::UserPrivilege::{Admin, Member};
use common::{Client, ClientID, ClientPacket, ClientSession, ServerDB, ServerEvent, ServerState, UserInfo};
use rand::{rng, Rng};
use std::collections::HashMap;
use std::fs;
use std::fs::OpenOptions;
use std::io::{stdin, BufWriter, Write};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::sync::{Arc};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::{mpsc, Semaphore};
use tokio::sync::mpsc::{Sender, Receiver};

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();

    let (server_sender, server_recv) = mpsc::channel::<ServerEvent>(Semaphore::MAX_PERMITS);

    let sender_reference = Arc::new(server_sender);

    let server_state = handle_server_start(server_recv).await;

    let sender_ref = Arc::clone(&sender_reference);

    tokio::spawn(async move {
        server_input_handler(sender_ref).await;
    });

    tokio::spawn(async move {
        server_handler(server_state).await;
    });

    loop {
        let (socket, _) = listener.accept().await.unwrap();


        let sender = Arc::clone(&sender_reference);
        tokio::spawn(async move {
            handle_new_connection(socket, sender).await;
        });
    }
}

async fn handle_new_connection(stream: TcpStream, server_sender: Arc<Sender<ServerEvent>>) {
    let (sender, mut receiver) = mpsc::channel::<ServerEvent>(100);
    let mut stream = BufReader::new(stream);

    let mut line = String::new();

    stream.read_line(&mut line).await.unwrap();

    match serde_json::from_str::<ClientPacket>(&line).unwrap() {
        ClientPacket::IdentityRequest { .. } => {
            stream.write("Enter username: \n".as_bytes()).await.unwrap();
        }
        _ => {

        }
    }

    line = String::new();

    let byte_val = stream.read_line(&mut line).await.unwrap();

    let (reader_stream, mut writer_stream) = stream.into_inner().into_split();

    if byte_val == 0 {
        println!("Client disconnected during handshake. Connection information: IP: {}",writer_stream.peer_addr().unwrap().ip());
        return;
    }

    let packet: ClientPacket = serde_json::from_str(&line).unwrap_or_else(|_| {
        match &line.lines().next().unwrap().split(" ").collect::<Vec<_>>()[..] {
            ["GET", resource, "HTTP/1.1"] => HTTPRequest {
                resource: resource.to_string(),
            },
            _ => HTTPRequest {
                resource: String::from("/")
            },
        }
    });


    let sender_ref = Arc::new(sender);
    match packet {
        ClientPacket::Connect { user } => {
            let user = UserInfo { username: user.username.trim_ascii().to_string(), password: user.password };
            server_sender.send(ServerEvent::ConnectionRequest { user, sender: Arc::clone(&sender_ref), }).await.unwrap();
        }
        _ => {}
    }

    loop {
        if let Some(response) = receiver.recv().await {
            if let ConnectionAccepted { client_id, user } = response {
                let client = Client { client_id: client_id.clone(), privilege: Member, };

                let client_ref = Arc::new(client.clone());
                tokio::spawn(async move {
                    reader_socket(reader_stream, Arc::clone(&server_sender), Arc::clone(&client_ref)).await;
                });

                tokio::spawn(async move  {
                    writer_socket(receiver, writer_stream).await;
                });
                sender_ref.send(ConnectionAccepted {client_id, user}).await.unwrap();
            } else if let HTTPResponse { status, contents, .. } = response {
                let response = format!("{status}\r\nLocation: {contents}\r\n\r\nContent-Type: text/plain; charset=utf-8");
                writer_stream.write_all(response.as_bytes()).await.unwrap();
            } else if let ConnectionRejected {reason} = response {
                writer_stream.write_all(serde_json::to_string(&ConnectionReject { reason }).unwrap().as_bytes()).await.unwrap();
                writer_stream.write_all(b"\n").await.unwrap();
            }
            break;
        }
    }
}

async fn reader_socket(stream: OwnedReadHalf, server_sender: Arc<Sender<ServerEvent>>, client: Arc<Client>) {
    let mut stream = BufReader::new(stream);
    loop {
        let mut line = String::new();

        let byte_val = stream.read_line(&mut line).await.unwrap();

        if byte_val == 0 { server_sender.send(ServerEvent::UserDisconnected { user: client }).await.unwrap();
            break;
        }

        let packet: ClientPacket = serde_json::from_str(&line).unwrap();

        let event = packet_to_event(packet, Arc::clone(&client));

        server_sender.send(event).await.unwrap();
    }
}

fn packet_to_event(packet: ClientPacket, client: Arc<Client>) -> ServerEvent {
    match packet {
        ClientPacket::ChatMessage { contents } => ChatMessageReceive { from: Arc::clone(&client), contents, },
        ClientPacket::PrivateMessage { to, contents, .. } => PrivateMessage { to, from: Arc::clone(&client), contents, },
        _ => Error {
            message: "Error".to_string(),
        },
    }
}

async fn writer_socket(mut receiver: Receiver<ServerEvent>, mut stream: OwnedWriteHalf) {
    loop {
        if let Some(event) = receiver.recv().await {
            let contents = serde_json::to_string(&event_to_packet(event)).unwrap();
            stream.write_all(contents.as_bytes()).await.unwrap();
            stream.write_all(b"\n").await.unwrap() ;
        }
    }

}

fn event_to_packet(server_event: ServerEvent) -> ClientPacket {
    match server_event {
        Broadcast { contents } => ClientPacket::ChatMessage { contents },
        ServerEvent::Message { contents } => {
            ClientPacket::ChatMessage { contents  }
        },
        ConnectionAccepted {client_id, ..} => {
            ClientPacket::IdentityInfo {information: format!("Assigned identity: {}, ID: {}", client_id.username, client_id.id)}
        },
        _ => Disconnect,
    }
}

async fn handle_server_shutdown(server_db: &ServerDB) {
    println!("Shutting down server.");

    let as_json = serde_json::to_string(&server_db.users).unwrap();

    let mut writer = BufWriter::new(OpenOptions::new().write(true).open("./users.json").unwrap());

    writer.write_all(as_json.as_bytes()).unwrap();
}

async fn handle_server_start(server_recv: Receiver<ServerEvent>) -> ServerState {
    let mut server_state = ServerState::new(server_recv);

    let current_users: Vec<UserInfo> = serde_json::from_str(&fs::read_to_string("./users.json").unwrap()).unwrap_or(Vec::new());

    server_state.db.users = current_users;

    server_state
}

async fn server_input_handler(server_sender: Arc<Sender<ServerEvent>>) {
    loop {
        let mut cmd = String::new();

        stdin().read_line(&mut cmd).unwrap();

        match cmd.trim_ascii() {
            "/shutdown" => {
                server_sender.send(Shutdown).await.unwrap();
                break;
            }
            _ => {

            }
        }
    }
}

async fn server_handler(mut server: ServerState) {
    let server_identity = Arc::new(Client { client_id: ClientID { username: String::from("Server"), id: 0  }, privilege: Admin });
    loop {
        if let Some(event) = server.receiver.recv().await {
            match event {
                ServerEvent::ConnectionRequest { user, sender } => {
                    println!("Connection request from: {}", user.username.trim_ascii());
                    let reference = Arc::new(user.clone());
                    let db_ref = Arc::new(&server.db);
                    let accepted = handle_client_connect(Arc::clone(&reference), sender, db_ref, &mut server.clients).await;
                    if accepted.is_some() && !server.db.users.contains(&user) {
                        server.db.users.push(user);
                    } else if accepted.is_none() {
                        println!("Invalid password passed for user: {}", user.username.trim_ascii());
                    }
                }
                ChatMessageReceive { from, contents } => {
                    handle_broadcast(contents, from, &server.clients.values().collect::<Vec<_>>()).await;
                }
                ServerEvent::UserDisconnected { user } => {
                    println!("{} has disconnected", user.client_id.username);
                    handle_client_disconnect(user, Arc::clone(&server_identity), &mut server.clients).await;
                }
                PrivateMessage { to, from, contents } => {
                    let username = to.split("#").collect::<Vec<_>>().get(0).unwrap().to_string();

                    match find_user(&username,  &server.clients) {
                        Some(client) => {
                            handle_pm(&client, from, contents, &server.clients).await;
                        }
                        None => {
                            server.clients.get(&from).unwrap().sender.send(Error { message: String::from("User not found"), }).await.unwrap();
                        }
                    }
                }
                ServerEvent::HTTPRequest {  sender, .. } => {
                    let contents =
                        "https://jojeecs.github.io/portfolio/".to_string();
                    let length = contents.len();
                    let status = String::from("HTTP/1.1 307 Temporary Redirect");

                    sender.send(HTTPResponse { status, contents, length, }).await.unwrap();
                },
                Shutdown => {
                    handle_server_shutdown(&server.db).await;
                    std::process::exit(0);
                }
                _ => {}
            }
        }
    }
}

fn find_user<'a>(username: &String, clients: &'a HashMap<Client, ClientSession>) -> Option<&'a Client> {
    for client in clients.keys().collect::<Vec<_>>() {
        if client.client_id.username.eq(username) {
            return Some(&client);
        }
    }

    None
}

async fn handle_pm(to: &Client, from: Arc<Client>, message: String, current_users: &HashMap<Client, ClientSession>, ) {
    let formatted = format!("From {}: {}", from.client_id.username, message);
    current_users.get(to).unwrap().sender.send(ServerEvent::Message { contents: formatted, }).await.unwrap();
}

fn assign_identity(user: &UserInfo, current_ids: &HashMap<Client, ClientSession>, ) -> Option<Client> {
    let user_id = ClientID { username: user.username.trim_ascii().to_string(),  id: rng().next_u64() as usize, };
    let id = Client { client_id: user_id, privilege: Member, };

    if current_ids.contains_key(&id) {
        return assign_identity(&user, current_ids);
    }

    Some(id)
}

async fn handle_client_connect(user: Arc<UserInfo>, sender: Arc<Sender<ServerEvent>>, server_db: Arc<&ServerDB>, clients: &mut HashMap<Client, ClientSession>) -> Option<bool> {
    if server_db.users.contains(&user) {
        let other = server_db.users.iter().find(|other| { other.username == user.username }).unwrap();
        if !other.password.eq_ignore_ascii_case(&user.password) {
            sender.send(ConnectionRejected { reason: "Invalid Password".to_string() }).await.unwrap();
            return None;
        }
    }
    let new_client = assign_identity(&user, &clients);
    if new_client.is_some() {
        let client = new_client.unwrap();
        let id = client.client_id.clone();
        sender.send(ConnectionAccepted { client_id: id.clone(), user }).await.unwrap();
        clients.insert(client, ClientSession { client_id: id, sender }, );
        Some(true)
    } else {
        sender.send(ConnectionRejected { reason: "Incorrect username formatting".to_string() }).await.unwrap();
        None
    }
}
async fn handle_client_disconnect(client: Arc<Client>, server_identity: Arc<Client>,  clients: &mut HashMap<Client, ClientSession>) {
    clients.remove(&client).unwrap();
    handle_broadcast(format!("{} has disconnected.", client.client_id.username), server_identity, &clients.values().collect::<Vec<_>>()).await;
}

async fn handle_broadcast(contents: String, originator: Arc<Client>, clients: &Vec<&ClientSession>) {
    if contents.is_empty() {
        return;
    }

    let formatted;

    if originator.client_id.id != 0 {
        formatted = format!("{}: {}", originator.client_id.username, contents.trim_ascii());
    } else {
        formatted = format!("SERVER: {}", contents.trim_ascii());
    }

    for session in clients {
        if session.client_id.id == originator.client_id.id {
            continue;
        } else {
            session.sender.send(Broadcast { contents: formatted.clone(), }).await.unwrap();
        }
    }
}
