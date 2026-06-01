use std::thread::sleep;
use common::ClientPacket::{Disconnect, HTTPRequest};
use common::ServerEvent::{Broadcast, ChatMessageReceive, ConnectionAccepted, ConnectionRejected, Error, HTTPResponse, PrivateMessage, };
use common::UserPrivilege::{Admin, Member};
use common::{Client, ClientID, ClientPacket, ClientSession, ServerEvent, ServerState};
use rand::{rng, Rng, RngExt};
use std::collections::HashMap;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use std::sync::{Arc};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Sender, Receiver};

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();

    let (server_sender, server_recv) = mpsc::channel::<ServerEvent>(100);

    let server_state = ServerState::new(server_recv);

    tokio::spawn(async move {
        server_handler(server_state).await;
    });

    let sender_reference = Arc::new(server_sender);

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
        ClientPacket::IdentityRequest { id } => {
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


    match packet {
        ClientPacket::Connect { username } => {
            server_sender.send(ServerEvent::ConnectionRequest { username: username.trim_ascii().to_string(), sender, }).await.unwrap();
        }
        HTTPRequest { resource } => {
            let sender = Arc::new(sender);
            let mut resource = resource.strip_prefix("/").unwrap().to_string();
            if resource.is_empty() {
                resource = String::from("index.html");
            }
            server_sender.send(ServerEvent::HTTPRequest { resource: resource.clone(), sender: Arc::clone(&sender), }).await.unwrap();
        }
        _ => {}
    }

    loop {
        if let Some(response) = receiver.recv().await {
            if let ConnectionAccepted { client_id } = response {
                let client = Client { client_id, privilege: Member, };

                tokio::spawn(async move {
                    reader_socket(reader_stream, Arc::clone(&server_sender), Arc::new(client)).await;
                });

                tokio::spawn(async move  {
                    writer_socket(receiver, writer_stream).await;
                });
            } else if let HTTPResponse { status, contents, .. } = response {
                let response = format!("{status}\r\nLocation: {contents}\r\n\r\nContent-Type: text/plain; charset=utf-8");
                writer_stream.write_all(response.as_bytes()).await.unwrap();
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
            stream.write_all(serde_json::to_string(&event_to_packet(event)).unwrap().as_bytes()).await.unwrap();
            stream.write_all(b"\n").await.unwrap();
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

async fn server_handler(mut server: ServerState) {
    let server_identity = Arc::new(Client { client_id: ClientID { chat_tag: 9999, username: String::from("Server"), id: 0  }, privilege: Admin });
    loop {
        if let Some(event) = server.receiver.recv().await {
            match event {
                ServerEvent::ConnectionRequest { username, sender } => {
                    println!("Connection request from: {username}");
                    handle_client_connect(username, sender, &mut server.clients).await;
                }
                ChatMessageReceive { from, contents } => {
                    handle_broadcast(contents, from, &server.clients.values().collect::<Vec<_>>()).await;
                }
                ServerEvent::UserDisconnected { user } => {
                    println!("{}#{} has disconnected", user.client_id.username, user.client_id.chat_tag);
                    handle_client_disconnect(user, Arc::clone(&server_identity), &mut server.clients).await;
                }
                PrivateMessage { to, from, contents } => {
                    let username = to.split("#").collect::<Vec<_>>().get(0).unwrap().to_string();
                    let tag = to.split("#").collect::<Vec<_>>().get(1).unwrap().to_string().parse::<usize>().unwrap();

                    match find_user(&username, tag, &server.clients) {
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
                }
                _ => {}
            }
        }
    }
}

fn find_user<'a>(username: &String, tag: usize, clients: &'a HashMap<Client, ClientSession>) -> Option<&'a Client> {
    for client in clients.keys().collect::<Vec<_>>() {
        if client.client_id.username.eq(username) && client.client_id.chat_tag.eq(&tag) {
            return Some(&client);
        }
    }

    None
}

async fn handle_pm(to: &Client, from: Arc<Client>, message: String, current_users: &HashMap<Client, ClientSession>, ) {
    let formatted = format!("From {}#{}: {}", from.client_id.username, from.client_id.chat_tag, message);
    current_users.get(to).unwrap().sender.send(ServerEvent::Message { contents: formatted, }).await.unwrap();
}

fn assign_identity(username: String, current_ids: &HashMap<Client, ClientSession>, ) -> Option<Client> {
    let user_id = ClientID { username: username.trim_ascii().to_string(), chat_tag: rng().random_range(1000..10000), id: rng().next_u64() as usize, };
    let id = Client { client_id: user_id, privilege: Member, };

    if current_ids.contains_key(&id) {
        return assign_identity(username, current_ids);
    }

    Some(id)
}

async fn handle_client_connect(username: String, sender: Sender<ServerEvent>, clients: &mut HashMap<Client, ClientSession>, ) {
    let new_client = assign_identity(username, &clients);
    if new_client.is_some() {
        let client = new_client.unwrap();
        let id = client.client_id.clone();
        sender.send(ConnectionAccepted { client_id: id.clone(), }).await.unwrap();
        clients.insert(client, ClientSession { client_id: id, sender, }, );
    } else {
        sender.send(ConnectionRejected { reason: "Incorrect username formatting".to_string() }).await.unwrap();
    }
}
async fn handle_client_disconnect(client: Arc<Client>, server_identity: Arc<Client>,  clients: &mut HashMap<Client, ClientSession>) {
    clients.remove(&client).unwrap();
}

async fn handle_broadcast(contents: String, originator: Arc<Client>, clients: &Vec<&ClientSession>) {
    if contents.is_empty() {
        return;
    }

    let formatted;

    if originator.client_id.id != 0 {
        formatted = format!("{}#{}: {}", originator.client_id.username, originator.client_id.chat_tag, contents.trim_ascii());
        println!("Broadcasting message \"{formatted}\"");
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
