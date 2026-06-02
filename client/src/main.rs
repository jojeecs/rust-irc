use std::io::{stdin};
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

        let response = handshake(&mut reader).await;

        if response.is_none() {
            return;
        }

        let response = response.unwrap();

        if let Ok(serialized) = serde_json::to_string(&Connect {user: response}) {
            let _ = write_stream.write_all(serialized.as_bytes()).await;
            let _ =write_stream.write_all(b"\n").await;
        }

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

    if let Err(e) = reader.read_line(&mut buf).await {
        eprintln!("Failed to read from stream: {}", e);
        return None;
    }

    let mut username = String::new();

    print!("{}", buf);

    if let Err(e) = stdin().read_line(&mut username) {
        eprintln!("Error reading username: {}", e);
    }
    while !verify_username(&username) {
        username.clear();
        if let Err(e) = stdin().read_line(&mut username) {
            eprintln!("Error reading username: {}", e);
        }
    }

    username = username.trim().to_string();

    let cfg = rpassword::ConfigBuilder::new().password_feedback_mask('*').build();

    let prompt = format!("Enter password for {}", username);
    let password = match rpassword::prompt_password_with_config(prompt, cfg) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return None
        }
    };

    let mut hasher = Sha3_256::new();
    hasher.update(password.as_bytes());
    let hash = hasher.finalize();

    let mut password_hash = String::new();


    for byte in hash {
        password_hash.push_str(&format!("{:02x}", byte));
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


        let byte_val = match stream.read_line(&mut str_buffer).await {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Error reading from stream: {}", e);
                continue;
            }
        };

        if byte_val == 0 {
            println!("Server shutting down.");
            std::process::exit(0);
        }


        let packet: ClientPacket =  match serde_json::from_str(&str_buffer) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Error parsing packet: {}", e);
                continue;
            }
        };

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

        if let Err(e) = stdin().read_line(&mut message) {
            eprintln!("Error reading input: {}", e);
        }

        let packet = raw_msg_to_packet(message.clone());

        if let Ok(serialized) = serde_json::to_string(&packet) {
            let _ = stream.write_all(serialized.as_bytes()).await;
            let _ =stream.write_all(b"\n").await;
        }
        if message.trim_ascii().eq("/exit") {
            std::process::exit(0);
        }
    }
}

fn raw_msg_to_packet(raw_msg: String) -> ClientPacket {
    if raw_msg.starts_with("/") {
        let mut split = raw_msg.split(" ").collect::<Vec<_>>();
        if let Some(cmd) = split.remove(0).strip_prefix("/") {
            if cmd.eq("pm") {
                let user = split.remove(0);

                let message = split.join(" ");

                return PrivateMessage { to: user.to_string(), contents: message };
            } else if cmd.trim_ascii() == "exit" {
                return Disconnect;
            }
        }
    }else {
        return ChatMessage { contents: raw_msg };
    }

    Disconnect
}