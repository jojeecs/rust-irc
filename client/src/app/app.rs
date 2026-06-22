use crossterm::event::Event::Key;
use tokio::sync::mpsc::{UnboundedSender};
use ratatui::DefaultTerminal;
use tokio::sync::mpsc::UnboundedReceiver;
use common::ClientPacket;
use common::ClientPacket::RoomChange;
use common::room::room::Room;
use crate::event::{Event, EventHandler};
use crate::pages::home_page::home_page::HomePage;
use crate::state::action::Action;
use crate::state::state::{ConnectionState, LoginState};
use crate::ui_management::ui_manager::{Screen, UiManager};

pub struct Client<'a> {
    ui_manager: UiManager<'a>,
    connection_state: ConnectionState,
    socket_tx: UnboundedSender<ClientPacket>,
    ui_rx: UnboundedReceiver<Action>,
    events: EventHandler,
    rooms: Vec<String>,
}

impl<'a> Client<'a> {
    pub fn new(socket_tx: UnboundedSender<ClientPacket>) -> (Self, UnboundedSender<Action>)  {
        let (ui_manager, ui_rx, ui_tx) = UiManager::new();

        (Self {
            ui_manager,
            connection_state: ConnectionState { connected: true },
            socket_tx,
            ui_rx,
            events: EventHandler::new(),
            rooms: Vec::new(),
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
                        },
                        crossterm::event::Event::Resize(x, y) => {
                            self.ui_manager.handle_resize(x, y);
                        },
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
                            if contents.starts_with("/pm") {
                                let to_username = contents.split(" ").collect::<Vec<_>>().get(1).unwrap_or(&"").to_string();
                                let contents = contents.split(" ").collect::<Vec<_>>().split_at(2).1.join(" ");
                                self.socket_tx.send(ClientPacket::PrivateMessage { to: to_username, contents })?;
                                continue;
                            }
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
                        ClientPacket::AuthenticationAccepted { new_user } => {
                            self.ui_manager.switch_screen(Screen::Home(HomePage::new(self.ui_manager.app_tx.clone())))
                        }
                        ClientPacket::PublicMessage {contents} | ClientPacket::PrivateMessage { contents, .. } => {
                            self.ui_manager.handle_msg(contents);
                        },
                        ClientPacket::RoomUpdate {rooms} => {
                            for room in rooms {
                                if !self.rooms.contains(&room) {
                                    self.rooms.push(room);
                                }
                            }
                        }
                        _ => {}
                    }
                },
                Action::ServerConnectionAccepted => {
                    self.ui_manager.signal_connection();
                },
                Action::ServerConnectionFailed => {
                    self.ui_manager.current_screen.add_notification(LoginState::SERVER_DOWN);
                },
                Action::RoomChange {new_room_name, old_room_name} => {
                    let _ = self.socket_tx.send(RoomChange {new_room_name, old_room_name});
                },
                Action::RoomCreationAttempt {name} => {

                }
                _ => {}
            }
        }
    }
}
