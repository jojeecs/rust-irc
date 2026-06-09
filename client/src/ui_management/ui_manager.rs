use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;
use tokio::sync::{mpsc, Semaphore};
use tokio::sync::mpsc::{Receiver, Sender, UnboundedReceiver, UnboundedSender};
use tui_textarea::Key;
use crate::pages::login_page::login_page::LoginPage;
use crate::state::action::Action;

pub enum Screen {
    Login(LoginPage),
    Home,
    Settings,
}

pub trait Page {
    fn draw(&self, frame: &mut Frame, area: Rect);
    fn handle_event(&mut self, event: KeyEvent);
}

pub struct UiManager {
    pub app_tx: UnboundedSender<Action>,
    pub current_screen: Screen,
}

impl UiManager {
    pub fn new() -> (Self, UnboundedReceiver<Action>, UnboundedSender<Action>) {
        let (app_tx, app_rx) = mpsc::unbounded_channel::<Action>();

        (Self { app_tx: app_tx.clone(), current_screen: Screen::Login(LoginPage::new(app_tx.clone())) }, app_rx, app_tx)
    }

    pub fn handle_input(&mut self, event: KeyEvent) {
        self.current_screen.handle_input(event);
    }

    pub async fn send_action(&self, action: Action) -> color_eyre::Result<()> {
        self.app_tx.send(action)?;
        Ok(())
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        match &self.current_screen {
            Screen::Login(login) => {
                login.draw(frame, frame.area());
            }
            _ => {}
        }
    }

    pub fn switch_screen(&mut self, new_screen: Screen) {
        self.current_screen = new_screen;
    }
}

impl Screen {
    pub fn handle_input(&mut self, event: KeyEvent) {
        match self {
            Screen::Login(login) => {
                login.handle_event(event);
            }
            _ => {}
        }
    }
}

