use std::io::{stdin};
use std::time::Duration;
use sha3::{Digest, Sha3_256};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use common::{ClientPacket, UserInfo};
use common::ClientPacket::{ChatMessage, Connect, Disconnect, PrivateMessage};
use tokio::net::{ TcpStream};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

#[tokio::main]
async fn main() {
    let str = TcpStream::connect("127.0.0.1:8080").await;
    if let Ok(stream) = str {
        let (read_stream, mut write_stream) = stream.into_split();

        write_stream.write_all(serde_json::to_string(&ClientPacket::IdentityRequest { id: 0 }).unwrap().as_bytes()).await.unwrap();
        write_stream.write_all(b"\n").await.unwrap();

        let mut reader = BufReader::new(read_stream);

        let response = handshake(&mut reader).await.unwrap();

        write_stream.write_all(serde_json::to_string(&Connect {user: response}).unwrap().as_bytes()).await.unwrap();
        write_stream.write_all(b"\n").await.unwrap();

        tokio::spawn(async move {
            write_socket(write_stream).await;
        });

        tokio::spawn(async move  {
            read_socket(reader.into_inner()).await;
        }).await.unwrap();
    } else {
        println!("Server is currently down. Please try connecting later.");
    }
}

async fn handshake(reader: &mut BufReader<OwnedReadHalf>) -> Option<UserInfo> {
    let mut buf = String::new();

    let bytes = reader.read_line(&mut buf).await.unwrap();


    if bytes == 0 {
        eprintln!("Server shutdown while performing handshake.");
        return None;
    }

    let mut username = String::new();

    print!("{}", buf);

    stdin().read_line(&mut username).unwrap();
    while !verify_username(&username) {
        username.clear();
        stdin().read_line(&mut username).unwrap();
    }

    let mut password = String::new();

    println!("Enter password for username: ");

    stdin().read_line(&mut password).unwrap();

    let mut hasher = Sha3_256::new();
    hasher.update(password.as_bytes());
    let hash = hasher.finalize();

    let mut password_hash = String::new();

    for byte in hash {
        password_hash.push_str(format!("{:02x}", byte).as_str());
    }
    let user = UserInfo { username: username.clone(), password: password_hash  };

    Some(user)
}

fn verify_username(username: &String) -> bool {
    if username.trim_ascii().is_empty() {
        println!("Usernames cannot be empty!");
        return false;
    } else if username.trim_ascii().contains(" ") {
        println!("Usernames cannot contain spaces!");
        return false;
    }

    true
}

async fn read_socket(stream: OwnedReadHalf) {
    let mut stream = BufReader::new(stream);
    loop {
        let mut str_buffer = String::new();

        let byte_val = stream.read_line(&mut str_buffer).await.unwrap();

        if byte_val == 0 {
            println!("Server shutting down.");
            std::process::exit(0);
        }

        let packet: ClientPacket = serde_json::from_str(&str_buffer).unwrap();

        match packet {
            ChatMessage {contents} => {
                println!(r"{}", contents.trim_ascii());
            },
            ClientPacket::IdentityInfo {information} => {
                println!(r"{}", information.trim_ascii());
            },
            ClientPacket::ConnectionReject {reason} => {
                println!("{reason}");
                std::process::exit(0);
            },
            Disconnect => {
                println!("Server shutting down.");
                std::process::exit(0);
            }
            _ => {
                println!("{packet:?}")
            }
        }
    }
}

async fn write_socket(mut stream: OwnedWriteHalf) {
    loop {
        let mut message = String::new();

        println!("Enter message or type /exit to leave chatroom");

        stdin().read_line(&mut message).unwrap();

        let packet = raw_msg_to_packet(message.clone());

        let serialized = serde_json::to_string(&packet).unwrap();

        stream.write_all(serialized.as_bytes()).await.unwrap();
        stream.write_all(b"\n").await.unwrap();
        if message.trim_ascii().eq("/exit") {
            std::process::exit(0);
        }
    }
}

fn raw_msg_to_packet(raw_msg: String) -> ClientPacket {
    if raw_msg.starts_with("/") {
        let mut split = raw_msg.split(" ").collect::<Vec<_>>();
        let cmd = split.remove(0).strip_prefix("/").unwrap();
        if cmd.eq("pm") {
            let user = split.remove(0);

            let message = split.join(" ");

            return PrivateMessage { to: user.to_string(), contents: message };
        }
        else if cmd.trim_ascii() == "exit" {
            return Disconnect;
        }
    } else {
        return ChatMessage { contents: raw_msg };
    }

    Disconnect
}