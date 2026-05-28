use std::io::{stdin, BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::{thread};
use std::time::Duration;
use common::{Delimiter, ServerEvent};

fn main() {
    let mut username = String::new();
    println!("Welcome to the chatroom!\nPlease enter a username to continue: ");

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
fn read_socket(mut stream: TcpStream) {
    let mut str_buffer = String::new();
    let mut reader = BufReader::new(stream);
    loop {
        let byte_val = reader.read_line(&mut str_buffer).unwrap();

        if byte_val == 0 {
            println!("Server shutting down.");
            std::process::exit(0);
        }

        let message = str_buffer.clone();

        println!("{message}");
    }
}

fn write_socket(mut stream: TcpStream, username: String) {
    stream.write_all(format!("{} {}", Delimiter::USERNAME, username).as_bytes()).unwrap();
    loop {
        let mut message = String::new();

        println!("Enter message or type /exit to leave chatroom");

        stdin().read_line(&mut message).unwrap();

        if message.eq_ignore_ascii_case("exit\n") {
            std::process::exit(0);
        }

        let payload = format!(r"{} {}\n", Delimiter::CHAT_MSG, message);

        stream.write_all(payload.as_bytes()).unwrap();
    }
}