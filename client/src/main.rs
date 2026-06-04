use cliclack::{input, password, select};
use common::ClientPacket::{Disconnect, LoginRequestPacket, PrivateMessage, PublicMessage};
use common::{ClientPacket, LoginInfo};
use sha3::{Digest, Sha3_256};
use std::io::stdin;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

#[tokio::main]
async fn main() {
    let str = TcpStream::connect("127.0.0.1:8080").await;
    if let Ok(stream) = str {
        let (read_stream, mut write_stream) = stream.into_split();
        let mut reader = BufReader::new(read_stream);
        loop {
            let mut username = String::new();
            let has_account = match select("Login or create account: ")
                .item("new", "Create new account", "")
                .item("existing", "Login", "")
                .interact()
            {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Error while selecting login method: {}", e);
                    return;
                }
            };

            match has_account {
                "new" => {
                    username = get_username("What would you like your username to be?");
                }
                "existing" => {
                    username = get_username("Enter username");
                }
                _ => {
                    println!("Invalid input.")
                }
            }

            if let Ok(serialized) =
                serde_json::to_string(&ClientPacket::InitialRequest { username })
            {
                let _ = write_stream.write_all(serialized.as_bytes()).await;
                let _ = write_stream.write_all(b"\n").await;
            } else {
                eprintln!("Error serializing session request packet");
                continue;
            }

            let mut response = String::new();

            let byte_val = match reader.read_line(&mut response).await {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("Error reading server response: {}", e);
                    return;
                }
            };

            if byte_val == 0 {
                println!("Server shutdown during handshake.");
                return;
            }

            let packet: ClientPacket = match serde_json::from_str(&response) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Error deserializing packet from stream: {}", e);
                    return;
                }
            };

            match packet {
                ClientPacket::InitialResponse { username, new_user } => {
                    println!("{new_user}");
                    if new_user && has_account.eq("existing") {
                        let restart = match select("That user does not exist! Would you like to create an account with that name, or restart?")
                            .item("create", "Create account", "")
                            .item("restart", "Restart process", "")
                            .interact() {
                            Ok(r) => r,
                            Err(e) => {
                                eprintln!("Error taking restart confirmation: {}", e);
                                continue;
                            }
                        };
                        if restart.eq("restart") {
                            continue;
                        }
                    }
                    let login_info = match init_user(username, new_user) {
                        Some(l) => l,
                        None => {
                            eprintln!("Error initiating new user information.");
                            continue;
                        }
                    };
                    if let Ok(serialized) = serde_json::to_string(&LoginRequestPacket {
                        username: login_info.username,
                        password: login_info.password,
                    }) {
                        let _ = write_stream.write_all(serialized.as_bytes()).await;
                        let _ = write_stream.write_all(b"\n").await;
                        break;
                    }
                }
                _ => {
                    eprintln!("Received incorrect response from server, please try again later.");
                    return;
                }
            }
        }

        tokio::spawn(async move {
            write_socket(write_stream).await;
        });

        tokio::spawn(async move {
            read_socket(reader.into_inner()).await;
        })
        .await
        .unwrap();
    } else {
        println!("Server is currently down. Please try connecting later.");
    }
}

fn init_user(username: String, new: bool) -> Option<LoginInfo> {
    let mut prompt = String::new();
    if new {
        prompt = "Enter password for new account: ".to_string()
    } else {
        prompt = "Enter password: ".to_string();
    }
    let mut hasher = Sha3_256::new();
    let mut pass = String::new();
    loop {
        let password_str = match password(&prompt).mask('*').interact() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Error taking password input: {}", e);
                break;
            }
        };
        if new {
            let password_confirm = match password("Confirm password").mask('*').interact() {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Error taking confirmation password input: {}", e);
                    continue;
                }
            };
            if password_confirm.eq(&password_str) {
                pass = password_str;
                break;
            } else {
                println!("Passwords do not match!");
                continue;
            }
        } else {
            pass = password_str;
            break;
        }
    }
    hasher.update(pass);
    let hash = hasher.finalize();

    let mut password_hash = String::new();

    for byte in hash {
        password_hash.push_str(&format!("{:02x}", byte));
    }

    Some(LoginInfo {
        username,
        password: password_hash,
    })
}

fn get_username(prompt: &str) -> String {
    let mut username = String::new();

    loop {
        username.clear();
        username = match input(&prompt).interact() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error in taking username: {}", e);
                return String::new();
            }
        };
        if verify_username(&username) {break}
    }

    username
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

        let packet: ClientPacket = match serde_json::from_str(&str_buffer) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Error parsing packet: {}", e);
                continue;
            }
        };

        match packet {
            PublicMessage { contents } => {
                println!(r"{}", contents.trim_ascii());
            }
            ClientPacket::ConnectionRejected { reason } => {
                println!("{reason}");
                std::process::exit(0);
            }
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
            let _ = stream.write_all(b"\n").await;
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

                return PrivateMessage {
                    to: user.to_string(),
                    contents: message,
                };
            } else if cmd.trim_ascii() == "exit" {
                return Disconnect;
            }
        }
    } else {
        return PublicMessage { contents: raw_msg };
    }

    Disconnect
}
