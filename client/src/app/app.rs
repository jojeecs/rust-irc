use crossterm::event::Event::Key;
use tokio::sync::mpsc::{UnboundedSender};
use ratatui::DefaultTerminal;
use tokio::sync::mpsc::UnboundedReceiver;
use common::ClientPacket;
use crate::event::{Event, EventHandler};
use crate::pages::home_page::home_page::HomePage;
use crate::state::action::Action;
use crate::state::state::{ConnectionState, LoginState};
use crate::ui_management::ui_manager::{Screen, UiManager};

pub struct Client {
    ui_manager: UiManager,
    connection_state: ConnectionState,
    socket_tx: UnboundedSender<ClientPacket>,
    ui_rx: UnboundedReceiver<Action>,
    events: EventHandler,
}

impl Client {
    pub fn new(socket_tx: UnboundedSender<ClientPacket>) -> (Self, UnboundedSender<Action>)  {
        let (ui_manager, ui_rx, ui_tx) = UiManager::new();

        (Self {
            ui_manager,
            connection_state: ConnectionState { connected: true },
            socket_tx,
            ui_rx,
            events: EventHandler::new()
        }, ui_tx)
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> color_eyre::Result<()> {
        while self.connection_state.connected {
            let _ = terminal.draw(|frame| self.ui_manager.draw(frame));

            match self.events.next().await? {
                Event::Crossterm(event) => {
                    match event {
                        Key(key_event) => {
                            self.ui_manager.handle_input(key_event, &mut self.events);
                        }
                        _ => {}
                    }
                }
                Event::ActionEvent(action) => {
                    match action {
                        Action::LoginAttempt {username, password} => {
                            if let Err(e) = self.socket_tx.send(ClientPacket::LoginRequestPacket {username, password}) {
                                eprintln!("Application error: {}", e);
                            }
                        },
                        Action::Exit => {
                            self.connection_state.connected = false;
                        },
                        Action::SendMessage {contents} => {
                            self.socket_tx.send(ClientPacket::PublicMessage {contents})?;
                        },
                        Action::SocketMessage {packet} => {
                            self.socket_tx.send(packet)?;
                        },
                        _ => {}
                    }
                }
                _ => {}
            }
            self.tick();
        }
        Ok(())
    }

    fn tick(&mut self) {
        if let Ok(pkt) = self.ui_rx.try_recv() {
            match pkt {
                Action::SocketMessage {packet} => {
                    match packet {
                        ClientPacket::AuthenticationRejected {..} => {
                            self.ui_manager.current_screen.add_notification(&LoginState::INCORRECT_INFORMATION);
                        },
                        ClientPacket::AuthenticationAccepted => {
                            self.ui_manager.switch_screen(Screen::Home(HomePage::new(self.ui_manager.app_tx.clone())))
                        }
                        ClientPacket::PublicMessage {contents} => {
                            self.ui_manager.handle_msg(contents);
                        }
                        _ => {}
                    }
                },
                Action::ServerConnectionAccepted => {
                    self.ui_manager.signal_connection();
                },
                Action::ServerConnectionFailed => {
                    self.ui_manager.current_screen.add_notification(LoginState::SERVER_DOWN);
                }
                _ => {}
            }
        }
    }
}