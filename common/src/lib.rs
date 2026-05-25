use std::sync::{mpsc, Arc, Mutex};
use std::sync::mpsc::{Receiver, Sender};
use crate::Signal::{Disconnect, Username};

pub const USERNAME_DELIMITER: &'static str = "\\USRNME\\";
pub const DISCONNECT_DELIMITER: &'static str = "\\DISCONNECT\\";


#[derive(Debug, Clone)]
pub struct Client {
    pub username: String,
    pub sender: Sender<Message>,
    pub id: usize,
}

#[derive(Debug)]
pub struct Server {
    pub capacity: usize,
    pub user_amount: usize,
    pub receiver: Receiver<Message>,
    pub clients: Arc<Mutex<Vec<Client>>>,
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
}

impl Message {
    pub fn parse_signal(&self) -> Option<Signal> {
        if self.contents.starts_with(USERNAME_DELIMITER) {
            return Some(Username);
        } else if self.contents.starts_with(DISCONNECT_DELIMITER) {
            return Some(Disconnect);
        }



        None
    }
}

impl Server {

    pub fn request_connection(sender: Sender<Message>, username: String, current_amount: usize) -> Option<Client> {
        let id = current_amount;
        Some(Client { username, sender, id })
    }

    pub fn create_connection() -> (Sender<Message>, Receiver<Message>) {
        mpsc::channel::<Message>()
    }
}