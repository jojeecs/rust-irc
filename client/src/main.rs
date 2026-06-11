use clavis::{EncryptedPacket, EncryptedReader, EncryptedStream, EncryptedWriter};
use common::{ClientPacket};
use tokio::io::{ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::{ mpsc};
use crate::app::app::Client;
use crate::state::action::Action;
use crate::state::action::Action::{ServerConnectionAccepted, ServerConnectionFailed, SocketMessage};

mod event;
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

    tokio::spawn(async move {
        let _ = wait_for_ip(ui_tx, socket_rx).await;
    });

    client.run(terminal).await?;

    ratatui::restore();
    Ok(())
}

async fn wait_for_ip(ui_tx: UnboundedSender<Action>, mut socket_rx: UnboundedReceiver<ClientPacket>) -> color_eyre::Result<()> {
    let str: TcpStream;
    loop {
        if let Some(pkt) = socket_rx.recv().await {
            match pkt {
                ClientPacket::ConnectRequest { ip } => {
                    if let Ok(stream) = TcpStream::connect(ip).await {
                        ui_tx.send(ServerConnectionAccepted)?;
                        str = stream;
                        break;
                    } else {
                        ui_tx.send(ServerConnectionFailed)?;
                    }
                }
                _ => {  }
            }
        }
    }
    handle(str, ui_tx, socket_rx).await?;

    Ok(())
}

async fn handle(
    stream: TcpStream,
    ui_tx: UnboundedSender<Action>,
    socket_rx: UnboundedReceiver<ClientPacket>,
) -> color_eyre::Result<()> {
    let encrypted = match EncryptedStream::new(stream, None).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error encrypting stream: {}", e);
            return Ok(());
        }
    };
    let (read_stream, write_stream) = encrypted.split();
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
    ui_tx: UnboundedSender<Action>,
) {
    loop {
        let packet: ClientPacket = match stream.read_packet().await {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Error reading packet: {}", e);
                continue;
            }
        };

        let _ = ui_tx.send(SocketMessage {packet});
    }
}

async fn write_socket(
    mut stream: EncryptedWriter<WriteHalf<TcpStream>>,
    mut socket_rx: UnboundedReceiver<ClientPacket>,
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
