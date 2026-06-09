use crate::ui::App;
use clavis::{EncryptedPacket, EncryptedReader, EncryptedStream, EncryptedWriter};
use common::ClientPacket::{
    ConnectionAccepted, ConnectionRejected, Handshake,
};
use common::HandshakePacket::{ServerLoginCheck};
use common::{ClientPacket};
use tokio::io::{ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{ mpsc};
use crate::app::app::Client;

mod event;
mod ui;
pub mod ui_management;
pub mod pages;
pub mod state;
pub mod app;
pub mod components;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    let (socket_tx, socket_rx) = mpsc::unbounded_channel::<ClientPacket>();

    let (client, ui_tx) = Client::new(socket_tx);
    let terminal = ratatui::init();
    client.run(terminal)?;

    ratatui::restore();

    // let (ui_tx, ui_rx) = mpsc::channel::<ClientPacket>(Semaphore::MAX_PERMITS);
    //
    // color_eyre::install()?;
    // let terminal = ratatui::init();
    // tokio::spawn(async move {
    //     let result = App::new(ui_rx, socket_tx).run(terminal).await;
    //     ratatui::restore();
    //     result
    // });
    //
    // let _ = match TcpStream::connect("127.0.0.1:8080").await {
    //     Ok(s) => {
    //         handle(s, ui_tx, socket_rx).await?;
    //     }
    //     Err(_) => {
    //         ratatui::restore();
    //         return Ok(());
    //     }
    // };
    //
    // ratatui::restore();
    //
    Ok(())
}

async fn handle(
    stream: TcpStream,
    ui_tx: Sender<ClientPacket>,
    mut socket_rx: Receiver<ClientPacket>,
) -> color_eyre::Result<()> {
    let encrypted = match EncryptedStream::new(stream, None).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error encrypting stream: {}", e);
            return Ok(());
        }
    };
    let (mut read_stream, mut write_stream) = encrypted.split();
    loop {
        match socket_rx.recv().await {
            Some(Handshake { handshake_packet }) => match handshake_packet {
                _ => {
                    write_stream
                        .write_packet(&Handshake { handshake_packet })
                        .await?;

                    if let Ok(Handshake { handshake_packet }) = read_stream.read_packet().await {
                        match handshake_packet {
                            ServerLoginCheck { correct_password } => {
                                if correct_password {
                                    ui_tx.send(ConnectionAccepted).await?;
                                    break;
                                } else {
                                    ui_tx
                                        .send(ConnectionRejected {
                                            reason: "Incorrect password".to_string(),
                                        })
                                        .await?;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            },
            Some(ClientPacket::Disconnect) => {
                return Ok(());
            }
            None => {
                println!("Received nothing.");
            }
            _ => {
                break;
            }
        }
    }
    tokio::spawn(async move {
        read_socket(read_stream, ui_tx).await;
    });

    tokio::spawn(async move {
        write_socket(write_stream, socket_rx).await;
    }).await?;

    Ok(())
}

async fn read_socket(
    mut stream: EncryptedReader<ReadHalf<TcpStream>>,
    ui_tx: Sender<ClientPacket>,
) {
    loop {
        let packet: ClientPacket = match stream.read_packet().await {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Error reading packet: {}", e);
                continue;
            }
        };

        let _ = ui_tx.send(packet).await;
    }
}

async fn write_socket(
    mut stream: EncryptedWriter<WriteHalf<TcpStream>>,
    mut socket_rx: Receiver<ClientPacket>,
) {
    loop {
        if let Some(msg) = socket_rx.recv().await {
            match msg {
                ClientPacket::Disconnect => {
                    std::process::exit(0);
                }
                _ => {
                    let _ = stream.write_packet(&msg).await;
                }
            }
        }
    }
}
