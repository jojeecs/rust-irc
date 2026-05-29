use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::sync::{Arc};
use std::sync::mpsc::{Receiver, Sender};
use rand::{RngExt};
use serde::{Deserialize, Serialize};
use crate::UserPrivilege::{Member};

#[derive(Debug, Clone)]
pub struct ClientSession {
    pub sender: Sender<ServerEvent>,
    pub client_id: ClientID,
}

#[derive(Clone, Debug, Eq, Hash)]
pub struct Client {
    pub client_id: ClientID,
    pub privilege: UserPrivilege,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ClientID {
    pub tag: usize,
    pub username: String,
    pub id: usize,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum UserPrivilege {
    Member,
    Moderator,
    Admin,
}

pub struct ServerState {
    pub clients: HashMap<Client, ClientSession>,
    pub receiver: Receiver<ServerEvent>,
}

#[derive(Clone, Debug)]
pub struct Message {
    pub contents: String,
    pub owner: Arc<Client>,
    pub message_id: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Tag {
    pub tag: usize,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientPacket {
    Connect { username: String },
    ChatMessage { contents: String },
    PrivateMessage { target: String, contents: String },
    Disconnect
}

#[derive(Debug)]
pub enum ServerEvent {
    Packet { client_id: ClientID, packet: ClientPacket },
    ConnectionRequest { username: String, sender: Sender<ServerEvent>  },
    ConnectionAccepted { client_id: ClientID },
    ConnectionRejected { reason: String },
    ChatMessage { from: String, contents: String },
    PrivateMessage { to: String, from: String, contents: String },
    UserDisconnected { username: String },
    Error { message: String }
}

impl ServerState {
    pub fn new(receiver: Receiver<ServerEvent>) -> ServerState {
        ServerState { clients: HashMap::new(), receiver }
    }
}

impl Tag {
    pub fn new() -> Tag {
        let tag = rand::rng().random_range(1000..10000);

        Tag { tag }
    }
}

impl PartialEq for Client {
    fn eq(&self, other: &Self) -> bool {
        self.client_id.id == other.client_id.id || (self.client_id.username == other.client_id.username && self.client_id.tag == other.client_id.tag)
    }
}

impl Display for Client {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}#{}, ID: {}", self.client_id.username, self.client_id.tag, self.client_id.id)
    }
}

impl Display for Tag {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.tag)
    }
}

impl Client {
    pub fn generate(username: &String, id: usize) -> Client {
        let username = ClientID { username: username.to_string(), tag: 0, id };
        Client { client_id: username, privilege: Member }
    }

    pub fn username_tag(&self) -> String {
        format!("{}#{}", self.client_id.username, self.client_id.tag).to_string()
    }
}
