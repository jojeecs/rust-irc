use std::fmt::{Display, Formatter};
use std::sync::{mpsc, Arc, Mutex};
use std::sync::mpsc::{Receiver, Sender};
use rand::{RngExt};
use crate::Command::PM;
use crate::Signal::{Connect, Disconnect, UserCommand};

// Delimiters, used when a stream writes to the server, and the server needs to recognize certain actions
pub const USERNAME_DELIMITER: &'static str = "\\USRNME/";
pub const DISCONNECT_DELIMITER: &'static str = "\\DISCONNECT/";
pub const CONNECT_DELIMITER: &'static str = "\\CONNECT/";

// Commands
pub const LEAVE_COMMAND: &'static str = "/exit";
pub const PM_COMMAND: &'static str = "/pm";


#[derive(Debug, Clone)]
pub struct Client {
    pub sender: Sender<Message>,
    pub id: ClientID,
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
    UserCommand,
}

pub struct Room {

}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tag {
    pub tag: usize
}
#[derive(Clone, Debug, Eq)]
pub struct ClientID {
    pub tag: Tag,
    pub username: String,
    pub id: usize,
}

pub enum Action {
    UserDisconnect(Arc<Client>),
    UserConnect(Arc<Client>),
    RoomChange(Room, Room, Arc<Client>),
    UsernameChange(String),
}

pub enum Command {
    PM(String),
    Leave,

}

impl Tag {
    pub fn new() -> Tag {

        let tag = rand::rng().random_range(1000..10000);

        Tag { tag }
    }
}

impl PartialEq for ClientID {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Display for ClientID {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}#{}, ID: {}", self.username, self.tag, self.id)
    }
}

impl Display for Tag {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.tag)
    }
}

impl ClientID {
    pub fn generate(username: String, id: usize) -> ClientID {
        ClientID { tag: Tag::new(), username, id }
    }

    pub fn username_tag(&self) -> String {
        format!("{}#{}", self.username, self.tag).to_string()
    }
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
        } else if contents.trim_ascii().starts_with("/") {
             return Some(UserCommand);
         }

        None
    }

    pub fn parse_command(contents: String) -> Option<Command> {
        if contents.trim_ascii().starts_with(PM_COMMAND) {
            if let Some(user) = contents.split(" ").collect::<Vec<_>>().get(1) {
                return Some(Command::PM(user.to_string()));
            }
        }


        None
    }

    pub fn parse_pm_message(contents: String) -> String {
        let mut message = String::new();

        let  content_vec = contents.split(" ").collect::<Vec<_>>();

        let content_vec1 = content_vec.split_at(2).1;

        message.push_str(content_vec1.to_vec().join("").as_str());

        message.trim_ascii().to_string()
    }
}

impl Server {
    pub fn request_connection(sender: Sender<Message>, id: ClientID) -> Option<Client> {
        Some(Client { sender, id})
    }

    pub fn create_connection() -> (Sender<Message>, Receiver<Message>) {
        mpsc::channel::<Message>()
    }
}