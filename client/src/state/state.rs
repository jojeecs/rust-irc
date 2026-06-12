use ratatui::text::Line;
use common::room::room::Room;
use crate::pages::home_page::home_page::HomeField;
use crate::pages::home_page::home_page::HomeField::MessageInput;
use crate::pages::login_page::login_page::{LoginField, LoginStatus};
use crate::pages::login_page::login_page::LoginStatus::{Connecting};

pub struct LoginState {
    pub status: LoginStatus,
    pub focused_field: LoginField,
    pub errors: Vec<String>,
    pub new_user: bool,
}

pub struct HomeState<'a> {
    pub current_room: Room,
    pub current_field: HomeField,
    pub messages: Vec<Line<'a>>,
}

pub struct ConnectionState {
    pub connected: bool,
}

impl<'a> HomeState<'a> {
    pub fn new(current_room: Room) -> Self {
        Self {
            current_room,
            current_field: MessageInput,
            messages: Vec::new(),
        }
    }
}


impl LoginState {
    pub const USERNAME_EMPTY_ERR: &'static str = "Username cannot be empty.";
    pub const PASSWORD_EMPTY_ERR: &'static str = "Password cannot be empty.";
    pub const INCORRECT_INFORMATION: &'static str = "Incorrect username or password.";
    pub const SERVER_DOWN: &'static str = "Unable to connect to server with given IP & port.";

    pub fn has_error(&self, error: &str) -> bool {
        self.errors.contains(&error.to_string())
    }

    pub fn remove_error(&mut self, error: &str) {
        self.errors.retain(|e| {
           !e.eq(error)
        });
    }

    pub fn add_error(&mut self, error: &str) {
        if !self.has_error(error) {
            self.errors.push(error.to_string());
        }
    }

    pub fn set_state(&mut self, status: LoginStatus) {
        self.status = status;
    }

    pub fn set_focus(&mut self, to: usize) {
        if to == LoginField::NEXT {
            self.focused_field = self.focused_field.next();
        } else if to == LoginField::PREVIOUS {
            self.focused_field = self.focused_field.previous();
        }
    }
}

impl Default for LoginState {
    fn default() -> Self {
        Self {
            focused_field: LoginField::IP,
            errors: Vec::default(),
            status: Connecting,
            new_user: false,
        }
    }
}