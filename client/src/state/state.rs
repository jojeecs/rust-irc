use crate::pages::login_page::login_page::{LoginField, LoginStatus};
use crate::pages::login_page::login_page::LoginStatus::Idle;

pub struct LoginState {
    pub status: LoginStatus,
    pub focused_field: LoginField,
    pub errors: Vec<String>
}

pub struct ConnectionState {
    pub connected: bool,
}

impl LoginState {
    pub const USERNAME_EMPTY_ERR: &'static str = "Username cannot be empty.";
    pub const PASSWORD_EMPTY_ERR: &'static str = "Password cannot be empty.";
    pub const INCORRECT_INFORMATION: &'static str = "Incorrect username or password.";

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
            focused_field: LoginField::Username,
            errors: Vec::default(),
            status: Idle
        }
    }
}