use cliclack::{input, password, select};
use common::ClientPacket::{Disconnect, LoginRequestPacket, PrivateMessage, PublicMessage};
use common::{ClientPacket, LoginInfo};
use sha3::{Digest, Sha3_256};
use std::io::stdin;
use clavis::{EncryptedPacket, EncryptedReader, EncryptedStream, EncryptedWriter};
use tokio::io::{ReadHalf, WriteHalf};
use tokio::net::TcpStream;

#[tokio::main]
async fn main() {
    let str = TcpStream::connect("127.0.0.1:8080").await;
    if let Ok(stream) = str {
        let encrypted = match EncryptedStream::new(stream, None).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error encyrpting stream: {}", e);
                return;
            }
        };
        let (mut read_stream, mut write_stream) = encrypted.split();
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


            if let Err(e) = write_stream.write_packet(&ClientPacket::InitialRequest {username}).await {
                eprintln!("Error sending packet: {}", e);
                return;
            }

            let packet: ClientPacket = match read_stream.read_packet().await {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("Error reading server response: {}", e);
                    return;
                }
            };


            match packet {
                ClientPacket::InitialResponse { username, new_user } => {
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
                    if let Err(e) = write_stream.write_packet(&LoginRequestPacket {username: login_info.username, password: login_info.password}).await {
                        eprintln!("Error sending login info to server: {}", e);
                    }
                    break;
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
            read_socket(read_stream).await;
        })
        .await
        .unwrap();
    } else {
        println!("Server is currently down. Please try connecting later.");
    }
}

fn init_user(username: String, new: bool) -> Option<LoginInfo> {
    let prompt ;
    if new {
        prompt = "Enter password for new account: ".to_string()
    } else {
        prompt = "Enter password: ".to_string();
    }
    let mut hasher = Sha3_256::new();
    let pass;
    loop {
        let password_str = match password(&prompt).mask('*').interact() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Error taking password input: {}", e);
                return None;
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

async fn read_socket(mut stream: EncryptedReader<ReadHalf<TcpStream>>) {
    loop {
        let packet: ClientPacket = match stream.read_packet().await {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Error reading packet: {}", e);
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

async fn write_socket(mut stream: EncryptedWriter<WriteHalf<TcpStream>>) {
    loop {
        let mut message = String::new();

        println!("Enter message or type /exit to leave chatroom");

        if let Err(e) = stdin().read_line(&mut message) {
            eprintln!("Error reading input: {}", e);
        }

        let packet = raw_msg_to_packet(message.clone());

        if let Err(e) = stream.write_packet(&packet).await {
            eprintln!("Error sending packet: {}", e);
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
