use std::io::{stdin, BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::{thread};
use common::{ClientPacket};

fn main() {
    let mut username = String::new();
    println!("Welcome to the chatroom!\nPlease enter a client_id to continue: ");

    stdin().read_line(&mut username).unwrap();

    let write_stream = TcpStream::connect("127.0.0.1:8080").unwrap();
    let read_stream = write_stream.try_clone().unwrap();

    thread::spawn(move || {
        write_socket(write_stream, username);
    });

    thread::spawn(move || {
        read_socket(read_stream);
    });

    loop {}
}
fn read_socket(stream: TcpStream) {
    let mut reader = BufReader::new(stream);
    loop {
        let mut str_buffer = String::new();
        let byte_val = reader.read_line(&mut str_buffer).unwrap();

        // if byte_val == 0 {
        //     println!("Server shutting down.");
        //     std::process::exit(0);
        // }

        println!("{}",str_buffer.trim_ascii());
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

        let packet = ClientPacket::ChatMessage {
            contents: message,
        };

        let serialized = serde_json::to_string(&packet).unwrap();

        stream.write_all(serialized.as_bytes()).unwrap();
        stream.write_all(b"\n").unwrap();
    }
}