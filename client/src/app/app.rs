use crossterm::event;
use crossterm::event::KeyEvent;
use tokio::sync::mpsc::{Receiver, Sender, UnboundedSender};
use ratatui::DefaultTerminal;
use tokio::sync::mpsc::UnboundedReceiver;
use common::ClientPacket;
use crate::state::action::Action;
use crate::state::state::{ConnectionState, LoginState};
use crate::ui_management::ui_manager::{Page, Screen, UiManager};

pub struct Client {
    ui_manager: UiManager,
    connection_state: ConnectionState,
    login_state: LoginState,
    socket_tx: UnboundedSender<ClientPacket>,
    ui_rx: UnboundedReceiver<Action>
}

impl Client {
    pub fn new(socket_tx: UnboundedSender<ClientPacket>) -> (Self, UnboundedSender<Action>)  {
        let (ui_manager, ui_rx, ui_tx) = UiManager::new();

        (Self {
            ui_manager,
            connection_state: ConnectionState { connected: true },
            login_state: LoginState::default(),
            socket_tx,
            ui_rx
        }, ui_tx)
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> color_eyre::Result<()> {
        while self.connection_state.connected {
            let _ = terminal.draw(|frame| self.ui_manager.draw(frame));

            let event = event::read()?;

            match event.as_key_event() {
                None => {}
                Some(key_event) => {
                    self.ui_manager.handle_input(key_event);
                }
            }
            self.tick();
        }
        Ok(())
    }

    fn tick(&mut self) {
        if let Ok(msg) = self.ui_rx.try_recv() {
            match msg {
                Action::Exit => {
                    self.connection_state.connected = false;
                },
                Action::LoginAttempt {username, password} => {
                    if let Err(e) = self.socket_tx.send(ClientPacket::LoginRequestPacket {username, password}) {
                        eprintln!("Application error: {}", e);
                    }
                },
                _ => {}
            }
        }
    }
}