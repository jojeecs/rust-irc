use std::io::{stdin, BufRead, BufReader, Write};
use std::net::TcpStream;
use std::{thread};
use common::{ClientPacket};
use common::ClientPacket::{ChatMessage, Disconnect, PrivateMessage};

fn main() {
    let mut username = String::new();
    println!("Welcome to the chatroom!\nPlease enter a username to continue: ");
    loop {
        stdin().read_line(&mut username).unwrap();
        if verify_username(&username) { break; }
        else { username.clear(); }
    }


    let write_stream = TcpStream::connect("127.0.0.1:8080");
    if let Ok(write_stream) = write_stream {
        let read_stream = write_stream.try_clone().unwrap();

        thread::spawn(move || {
            write_socket(write_stream, username);
        });

        thread::spawn(move || {
            read_socket(read_stream);
        });
    } else {
        println!("Server is currently down. Please try connecting later.");
    }
}

fn verify_username(username: &String) -> bool {
    if username.is_empty() {
        println!("Usernames cannot be empty!");
        return false;
    } else if username.contains(" ") {
        println!("Usernames cannot contain spaces!");
        return false;
    }

    true
}

fn read_socket(stream: TcpStream) {
    let mut reader = BufReader::new(stream);
    loop {
        let mut str_buffer = String::new();
        let byte_val = reader.read_line(&mut str_buffer).unwrap();

        if byte_val == 0 {
            println!("Server shutting down.");
            std::process::exit(0);
        }

        let packet: ClientPacket = serde_json::from_str(&str_buffer).unwrap();

        match packet {
            ChatMessage {contents} => {
                println!(r"{}", contents.trim_ascii());
            }
            _ => {
                println!("{packet:?}")
            }
        }
    }
}

fn write_socket(mut stream: TcpStream, username: String) {
    let username_packet = serde_json::to_string(&ClientPacket::Connect {username}).unwrap();
    stream.write_all(username_packet.as_bytes()).unwrap();
    stream.write_all(b"\n").unwrap();
    loop {
        let mut message = String::new();

        println!("Enter message or type /exit to leave chatroom");

        stdin().read_line(&mut message).unwrap();

        if message.eq_ignore_ascii_case("exit\n") {
            std::process::exit(0);
        }

        let packet = raw_msg_to_packet(message);


        let serialized = serde_json::to_string(&packet).unwrap();

        stream.write_all(serialized.as_bytes()).unwrap();
        stream.write_all(b"\n").unwrap();
    }
}

fn raw_msg_to_packet(raw_msg: String) -> ClientPacket {
    if raw_msg.starts_with("/") {
        let mut split = raw_msg.split(" ").collect::<Vec<_>>();
        let cmd = split.remove(0).strip_prefix("/").unwrap();
        let user = split.remove(0);

        let message = split.join(" ");

        if cmd.to_lowercase().eq("pm") {
            return PrivateMessage {to: user.to_string(), contents: message};
        } else if cmd.to_lowercase().eq("exit") {
            return Disconnect;
        }
    } else {
        return ChatMessage { contents: raw_msg };
    }


    Disconnect
}