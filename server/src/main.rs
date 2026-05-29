use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Arc};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use rand::{rng, Rng, RngExt};
use common::{ClientSession, Client, Message, ServerState, Tag, ClientPacket, ServerEvent, ClientID};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use common::ServerEvent::{ConnectionAccepted, ConnectionRejected};
use common::UserPrivilege::{Member};

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
    let writer_stream = stream.try_clone().unwrap();

    let mut reader = BufReader::new(stream);

    let byte_val = reader.read_line(&mut line).unwrap();

    if byte_val == 0 {
        println!("error");
    }

    println!("{}", line);

    let packet: ClientPacket = serde_json::from_str(&line).unwrap();

    println!("{packet:?}");

    match packet {
        ClientPacket::Connect {username} => {
            server_sender.send(ServerEvent::ConnectionRequest { username, sender }).unwrap();
        }
        _ => {

        }
    }

    loop {
        if let Ok(response) = receiver.recv() {
            if let ConnectionAccepted {client_id} = response {
                let client = Client { client_id, privilege: Member };

                thread::spawn(move || {
                    reader_socket(reader_stream, Arc::clone(&server_sender), Arc::new(client));
                });


                thread::spawn(move || {
                    writer_socket(receiver, writer_stream);
                });
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

        let packet: ClientPacket = serde_json::from_str(&line).unwrap();

    }
}


fn writer_socket(receiver: Receiver<ServerEvent>, mut stream: TcpStream) {
    loop {
        if let Ok(event) = receiver.recv() {
            
        }
    }
}

fn server_handler(mut server: ServerState) {
    loop {
        if let Ok(event) = server.receiver.recv() {
            match event {
                ServerEvent::ConnectionRequest {username, sender} => {
                    let new_client = assign_identity(username, &server.clients);
                    if new_client.is_some() {
                        let client = new_client.unwrap();
                        let id = client.client_id.clone();
                        sender.send(ConnectionAccepted { client_id: id.clone() }).unwrap();
                        server.clients.insert(client, ClientSession { client_id: id, sender  });
                    } else {
                        sender.send(ConnectionRejected {reason: "Incorrect username formatting".to_string()}).unwrap();
                    }
                }
                _ => {

                }
            }
        }
    }
}

fn find_user<'a>(username: &String, tag: usize, clients: &'a HashMap<Client, ClientSession>) -> Option<&'a Client> {
    for client in clients.keys().collect::<Vec<_>>() {
        if client.client_id.username.eq(username) && client.client_id.tag.eq(&tag) {
            return Some(&client);
        }
    }

    None
}

fn handle_pm(to: &Client, from: Arc<Client>, message: String, current_users: &HashMap<Client, ClientSession>) -> Option<()> {

    Some(())
}


fn assign_identity(username: String, current_ids: &HashMap<Client, ClientSession>) -> Option<Client> {
    let user_id = ClientID { username: username.clone(), tag: rng().random_range(1000..10000), id: rng().next_u64() as usize };
    let id = Client { client_id: user_id, privilege: Member };

    if current_ids.contains_key(&id) {
        return assign_identity(username, current_ids);
    }

    Some(id)
}

fn handle_client_connect(client_id: Client, client: ClientSession, clients: &mut HashMap<Client, ClientSession>) {
    clients.insert(client_id, client);
}
fn handle_client_disconnect(client: &Client, clients: &mut HashMap<Client, ClientSession>) {
    clients.remove(&client).unwrap();
}

fn handle_broadcast(message: Message, clients: &Vec<&ClientSession>) {
    if message.contents.is_empty() {
        return;
    }
    let originator = message.clone().owner;

    for session in clients {
        if session.client_id.id == originator.client_id.id {
            continue;
        } else {
        }
    }
}
