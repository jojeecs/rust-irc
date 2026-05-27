use std::io::{stdin, Read, Write};
use std::net::TcpStream;
use std::{io, thread};
use std::time::Duration;
use common::{CONNECT_DELIMITER, USERNAME_DELIMITER};

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
    let mut buffer = vec![0; 1024];
    loop {
        let byte_val = stream.read(&mut buffer).unwrap();

        if byte_val == 0 {
            std::process::exit(0);
        }

        let message = String::from_utf8_lossy(&buffer[0..byte_val]).to_string();

        println!("{message}");
    }
}

fn write_socket(mut stream: TcpStream, username: String) {
    stream.write_all(format!("{} / {}",USERNAME_DELIMITER, username).as_bytes()).unwrap();
    thread::sleep(Duration::from_millis(100));
    loop {
        let mut message = String::new();

        println!("Enter message or type exit to leave chatroom");

        stdin().read_line(&mut message).unwrap();

        if message.trim_ascii().eq_ignore_ascii_case("exit".trim_ascii()) {
            std::process::exit(0);
        }

        stream.write_all(message.as_bytes()).unwrap();
    }
}