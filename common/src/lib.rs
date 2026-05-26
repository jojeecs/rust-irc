use std::sync::{mpsc, Arc, Mutex};
use std::sync::mpsc::{Receiver, Sender};
use crate::Signal::{Connect, Disconnect, Username};

pub const USERNAME_DELIMITER: &'static str = "\\USRNME";
pub const DISCONNECT_DELIMITER: &'static str = "\\DISCONNECT";

pub const CONNECT_DELIMITER: &'static str = "\\CONNECT";


#[derive(Debug, Clone)]
pub struct Client {
    pub username: String,
    pub sender: Sender<Message>,
    pub id: usize,
}

#[derive(Debug)]
pub struct Server {
    pub receiver: Receiver<Message>,
    pub clients: Arc<Mutex<Vec<Client>>>,
}

pub struct ServerDB {
    pub capacity: usize,
    pub user_count: usize,
}

#[derive(Clone, Debug)]
pub struct Message {
    pub contents: String,
    pub owner: Arc<Client>,
    pub message_id: usize,
    pub signal: Option<Signal>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Signal {
    Disconnect,
    Username,
    Connect,
}

pub enum Action {
    UserDisconnect,
    UserConnect,
}

impl ServerDB {
    pub fn inc_user_count(&mut self) {
        self.user_count += 1;
    }

    pub fn dec_user_count(&mut self) {
        self.user_count -= 1;
    }
}

impl Message {
    pub fn parse_signal(contents: String) -> Option<Signal> {
         if contents.trim_ascii().starts_with(DISCONNECT_DELIMITER) {
            return Some(Disconnect);
        } else if contents.trim_ascii().starts_with(CONNECT_DELIMITER) {
            return Some(Connect);
        }

        None
    }
}

impl Server {

    pub fn request_connection(sender: Sender<Message>, username: String, id: usize) -> Option<Client> {
        Some(Client { username, sender, id })
    }

    pub fn create_connection() -> (Sender<Message>, Receiver<Message>) {
        mpsc::channel::<Message>()
    }
}