use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::sync::{mpsc, Arc, Mutex};
use std::sync::mpsc::{Receiver, Sender};
use rand::{RngExt};
use crate::Command::PM;

// Delimiters, used when a stream writes to the server, and the server needs to recognize certain actions
pub const USERNAME_DELIMITER: &'static str = "\\USRNME/";
pub const DISCONNECT_DELIMITER: &'static str = "\\DISCONNECT/";
pub const CONNECT_DELIMITER: &'static str = "\\CONNECT/";

// Commands
pub const LEAVE_COMMAND: &'static str = "/exit";
pub const PM_COMMAND: &'static str = "/pm";


#[derive(Debug, Clone)]
pub struct Client {
    pub sender: Sender<ServerEvent>,
    pub id: ClientID,
}

pub struct ServerState {
    pub db_sender: Sender<ServerEvent>,
    pub clients: HashMap<ClientID, Client>,
    pub receiver: Receiver<ServerEvent>,
    pub identity: Arc<ClientID>,
}

pub struct ServerDB {
    pub message_history: Vec<ServerEvent>,
    pub receiver: Receiver<ServerEvent>
}

#[derive(Clone, Debug)]
pub struct Message {
    pub contents: String,
    pub owner: Arc<ClientID>,
    pub message_id: usize,
}

#[derive(Debug)]
pub enum ServerEvent {
    ChatMessage(Message),
    Disconnect(Arc<ClientID>),
    Connect(Client),
    IdentityRequest(String, Arc<Sender<ServerEvent>>),
    IdentityAssignment(ClientID)
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Tag {
    pub tag: usize
}
#[derive(Clone, Debug, Eq, Hash)]
pub struct ClientID {
    pub tag: Tag,
    pub username: String,
    pub id: usize,
}

pub enum Command {
    PM(String),
    Leave,
}

impl ServerState {
    pub fn new(db_sender: Sender<ServerEvent>, receiver: Receiver<ServerEvent>) -> ServerState {
        let identity = Arc::new(ClientID { tag: Tag { tag: 0000 }, username: String::from("Server"), id: 0 });
        ServerState { db_sender, clients: HashMap::new(), receiver, identity }
    }
}

impl Tag {
    pub fn new() -> Tag {

        let tag = rand::rng().random_range(1000..10000);

        Tag { tag }
    }
}

impl PartialEq for ClientID {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id || (self.tag == other.tag && self.username.eq(&other.username))
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
    pub fn generate(username: &String, id: usize) -> ClientID {
        ClientID { tag: Tag::new(), username: username.to_string(), id }
    }

    pub fn username_tag(&self) -> String {
        format!("{}#{}", self.username, self.tag).to_string()
    }
}
impl Message {

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