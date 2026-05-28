use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::sync::{Arc};
use std::sync::mpsc::{Receiver, Sender};
use rand::{RngExt};

impl Delimiter<'static> {
    pub const USERNAME: Delimiter<'static> = Delimiter("USERNAME");
    pub const DISCONNECT: Delimiter<'static> = Delimiter("DISCONNECT");
    pub const CONNECT: Delimiter<'static> = Delimiter("CONNECT");
    pub const CMD_LEAVE: Delimiter<'static> = Delimiter("/exit");
    pub const CMD_PM: Delimiter<'static> = Delimiter("/pm");
    pub const CHAT_MSG: Delimiter<'static> = Delimiter("CHAT");
}


pub struct Delimiter<'a>(&'a str);

impl Display for Delimiter<'static> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

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
    IdentityAssignment(ClientID),
    Null,
}

pub enum Packet {
    Connect,
    GlobalChat,
    PrivateMessage,
    Disconnect,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Tag {
    pub tag: usize,
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

impl Packet {

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
        if contents.starts_with(Delimiter::CMD_PM.0) {
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

        message
    }
}