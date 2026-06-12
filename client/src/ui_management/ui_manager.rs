use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;
use tokio::sync::{mpsc};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use crate::event::Event::ActionEvent;
use crate::event::EventHandler;
use crate::pages::home_page::home_page::HomePage;
use crate::pages::login_page::login_page::LoginField::Username;
use crate::pages::login_page::login_page::LoginPage;
use crate::pages::login_page::login_page::LoginStatus::Idle;
use crate::state::action::Action;
use crate::state::action::Action::Exit;

pub enum Screen<'a> {
    Connect,
    Login(LoginPage),
    Home(HomePage<'a>),
    Settings,
}

pub trait Page {
    fn draw(&self, frame: &mut Frame, area: Rect);
    fn handle_event(&mut self, event: KeyEvent, event_handler: &mut EventHandler);
}

pub struct UiManager<'a> {
    pub app_tx: UnboundedSender<Action>,
    pub current_screen: Screen<'a>,
}

impl<'a> UiManager<'a> {
    pub fn new() -> (Self, UnboundedReceiver<Action>, UnboundedSender<Action>) {
        let (app_tx, app_rx) = mpsc::unbounded_channel::<Action>();

        (Self { app_tx: app_tx.clone(), current_screen: Screen::Login(LoginPage::new()) }, app_rx, app_tx)
    }

    pub fn handle_input(&mut self, event: KeyEvent, event_handler: &mut EventHandler) {
        self.current_screen.handle_input(event, event_handler);
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        match &self.current_screen {
            Screen::Login(login) => {
                login.draw(frame, frame.area());
            },
            Screen::Home(home) => {
                home.draw(frame, frame.area());
            }
            _ => {}
        }
    }

    pub fn handle_resize(&mut self, x: u16, y: u16) {
        self.current_screen.handle_resize(x, y);
    }

    pub fn switch_screen(&mut self, new_screen: Screen<'a>) {
        self.current_screen = new_screen;
    }

    pub fn handle_msg(&mut self, message: String) {
        self.current_screen.handle_msg(message);
    }

    pub fn signal_connection(&mut self) {
        self.current_screen.handle_successful_connection();
    }
}

impl<'a> Screen<'a> {
    pub fn handle_input(&mut self, event: KeyEvent, event_handler: &mut EventHandler) {
        match event.code {
            KeyCode::Esc => {
                event_handler.send(ActionEvent(Exit));
            },
            _ => {
                match self {
                    Screen::Login(login) => {
                        login.handle_event(event, event_handler);
                    },
                    Screen::Home(home) => {
                        home.handle_event(event, event_handler);
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn handle_successful_connection(&mut self) {
        match self {
            Screen::Login(login) => {
                login.state.status = Idle;
                login.state.focused_field = Username;
            }
            _ => {}
        }
    }

    pub fn handle_msg(&mut self, message: String) {
        match self {
            Screen::Home(home) => {
                home.state.messages.push(Line::from(message.clone()));
                home.message_box.new_msg(&message);
            }
            _ => {}
        }
    }

    pub fn add_notification(&mut self, error: &str) {
        match self {
            Screen::Login(login) => {
                login.state.add_error(error);
            }
            _ => {}
        }
    }

    pub fn handle_resize(&mut self, x: u16, y: u16) {
        match self {
            Screen::Home(home) => {
                home.message_box.calculate_lines(x.div_ceil(3) as usize, &home.state.messages);
            },
            _ => {}
        }
    }
}

