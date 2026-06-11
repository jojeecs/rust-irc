use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::Color::Red;
use ratatui::style::Style;
use ratatui::widgets::{Block, Paragraph, Wrap};
use tui_input::backend::crossterm::EventHandler as evt;
use common::ClientPacket::ConnectRequest;
use crate::components::input::{InputField};
use crate::event::{EventHandler};
use crate::event::Event::ActionEvent;
use crate::state::action::Action;
use crate::state::action::Action::{Exit, LoginAttempt};
use crate::state::state::LoginState;
use crate::ui_management::ui_manager::Page;

pub enum LoginField {
    IP,
    Username,
    Password,
}

pub enum LoginStatus {
    Connecting,
    Idle,
    Inputting,
    AttemptLogin,
    LoginFailed,
    LoginSuccessful,
}

pub struct LoginPage {
    pub state: LoginState,
    ip_input: InputField,
    username_input: InputField,
    password_input: InputField,
}

impl LoginPage {
    pub fn new() -> Self {
        let state = LoginState::default();
        let ip_input = InputField::new("Enter IP:Port to connect to: ".to_string());
        let username_input = InputField::new("Username".to_string());
        let password_input = InputField::new("Password".to_string()).set_hidden(true);

        Self { state, ip_input, username_input, password_input}
    }
}

impl Page for LoginPage {
    fn draw(&self, frame: &mut Frame, area: Rect) {
        let width = area.width.max(3) - 3;
        let error_messages = Paragraph::new(self.state.errors.join("\n")).style(Style::new().fg(Red)).wrap(Wrap {trim: true }).centered();
        let mut error_box = frame.area().centered(Constraint::Length(50), Constraint::Length(3));
        match self.state.status {
            LoginStatus::Connecting => {
                error_box.y -= 3;
                let ip_box = frame.area().centered(Constraint::Length(50), Constraint::Length(3));

                let ip_scroll = self.ip_input.input.visual_scroll(width as usize);

                let ip_input = Paragraph::new(self.ip_input.display())
                    .scroll((0, ip_scroll as u16))
                    .style(Style::default())
                    .block(Block::bordered().title(self.ip_input.label.clone()));

                frame.render_widget(ip_input, ip_box);

                let x = self.ip_input.input.visual_cursor().max(ip_scroll) - ip_scroll + 1;
                frame.set_cursor_position((ip_box.x + x as u16, ip_box.y + 1));
            }
            _ => {
                let mut username_box = frame.area().centered(Constraint::Length(50), Constraint::Length(3));
                let password_box = frame.area().centered(Constraint::Length(50), Constraint::Length(3));
                let mut help_box  = frame.area().centered(Constraint::Length(50), Constraint::Length(3));

                username_box.y -= 3;
                help_box.y += 3;
                error_box.y += 6;


                let username_scroll = self.username_input.input.visual_scroll(width as usize);
                let password_scroll = self.password_input.input.visual_scroll(width as usize);


                let username_input = Paragraph::new(self.username_input.display()).
                    scroll((0, username_scroll as u16))
                    .style(Style::default())
                    .block(Block::bordered().title(self.username_input.label.clone()));

                let password_input = Paragraph::new(self.password_input.display())
                    .scroll((0, password_scroll as u16))
                    .style(Style::default())
                    .block(Block::bordered().title(self.password_input.label.clone()));

                let help_message = Paragraph::new("Press <Enter> to submit").centered();


                frame.render_widget(username_input, username_box);
                frame.render_widget(password_input, password_box);
                frame.render_widget(help_message, help_box);


                match self.state.focused_field {
                    LoginField::Username => {
                        let x = self.username_input.input.visual_cursor().max(username_scroll) - username_scroll + 1;
                        frame.set_cursor_position((username_box.x + x as u16, username_box.y + 1));
                    }
                    LoginField::Password => {
                        let x = self.password_input.input.visual_cursor().max(password_scroll) - password_scroll + 1;
                        frame.set_cursor_position((password_box.x + x as u16, password_box.y + 1));
                    },
                    _ => {}
                }
            }
        }
        frame.render_widget(error_messages, error_box);

    }

    fn handle_event(&mut self, event: KeyEvent, event_handler: &mut EventHandler) {
        match event.code {
            KeyCode::Enter => {
                match self.state.status {
                    LoginStatus::Connecting => {
                        self.state.remove_error(LoginState::SERVER_DOWN);
                        event_handler.send(ActionEvent(Action::SocketMessage {packet: ConnectRequest {ip: self.ip_input.input.value_and_reset()}}));
                    },
                    _ => {
                        if self.username_input.is_empty() {
                            self.state.add_error(LoginState::USERNAME_EMPTY_ERR);
                        } else {
                            self.state.remove_error(LoginState::USERNAME_EMPTY_ERR);
                        }

                        if self.password_input.is_empty() {
                            self.state.add_error(LoginState::PASSWORD_EMPTY_ERR);
                        } else {
                            self.state.remove_error(LoginState::PASSWORD_EMPTY_ERR);
                        }

                        if self.state.errors.is_empty() {
                            event_handler.send(ActionEvent(LoginAttempt {
                                username: self.username_input.value(),
                                password: self.password_input.value(),
                            }))
                        }
                    }
                }
            },
            KeyCode::Tab => {
                match event.modifiers {
                    KeyModifiers::SHIFT => {
                        self.state.set_focus(LoginField::PREVIOUS);
                    }
                    _ => {
                        self.state.set_focus(LoginField::NEXT);
                    }
                }
            },
            _ => {
                self.state.remove_error(LoginState::INCORRECT_INFORMATION);
                let event = crossterm::event::Event::Key(event);
                match self.state.focused_field {
                    LoginField::Username => {
                        self.username_input.input.handle_event(&event);
                    },
                    LoginField::Password => {
                        self.password_input.input.handle_event(&event);
                    },
                    _ => {
                        self.ip_input.input.handle_event(&event);
                    }
                }
            }
        }
    }
}

impl LoginField {
    pub const NEXT: usize = 0;
    pub const PREVIOUS: usize = 1;

    pub fn next(&self) -> Self {
        match self {
            LoginField::Username => {
                LoginField::Password
            }
            LoginField::Password => {
                LoginField::Username
            },
            _ => {
                LoginField::IP
            }
        }
    }

    pub fn previous(&self) -> Self {
        match self {
            LoginField::Username => {
                LoginField::Password
            }
            LoginField::Password => {
                LoginField::Username
            },
            _ => {
                LoginField::IP
            }
        }
    }
}