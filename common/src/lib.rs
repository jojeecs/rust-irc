use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::sync::{Arc};
use serde::{Deserialize, Serialize};
use crate::UserPrivilege::{Member};
use tokio::sync::mpsc::{Sender, Receiver};

/// `ClientSession` holds information regarding a connection between client and server.
///
#[derive(Debug, Clone)]
pub struct ClientSession {
    pub sender: Arc<Sender<ServerEvent>>,
    pub client_id: ClientID,
}

/// `Client` holds information regarding a client
#[derive(Clone, Debug, Eq, Hash)]
pub struct Client {
    pub client_id: ClientID,
    pub privilege: UserPrivilege,
}

#[derive(Serialize, Deserialize, Debug, Eq, Clone)]
pub struct UserInfo {
    pub username: String,
    pub password: String,
}

/// `ClientID` is a support struct for `Client` that holds additional information.
/// This is mainly used for identifying `ClientSessions` in the server struct.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ClientID {
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
    pub db: ServerDB
}

pub struct ServerDB {
    pub users: Vec<UserInfo>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientPacket {
    Connect { user: UserInfo },
    ChatMessage { contents: String },
    PrivateMessage { to: String, contents: String },
    HTTPRequest {resource: String},
    IdentityRequest { id: usize},
    IdentityInfo {information: String},
    ConnectionReject {reason: String},
    Disconnect
}

#[derive(Debug)]
pub enum ServerEvent {
    ConnectionRequest { user: UserInfo, sender: Arc<Sender<ServerEvent>>  },
    ConnectionAccepted { client_id: ClientID, user: Arc<UserInfo> },
    ConnectionRejected { reason: String },
    ChatMessageReceive { from: Arc<Client>, contents: String },
    Broadcast { contents: String },
    PrivateMessage { to: String, from: Arc<Client>, contents: String },
    Message { contents: String },
    UserDisconnected { user: Arc<Client> },
    HTTPRequest {resource: String, sender: Arc<Sender<ServerEvent>>},
    HTTPResponse {status: String, contents: String, length: usize},
    Error { message: String },
    Shutdown,
}

impl ServerState {
    pub fn new(receiver: Receiver<ServerEvent>) -> ServerState {
        ServerState { clients: HashMap::new(), receiver, db: ServerDB::new()  }
    }
}

impl ServerDB {
    pub fn new() -> ServerDB {
        ServerDB { users: Vec::new() }
    }
}

impl PartialEq for Client {
    fn eq(&self, other: &Self) -> bool {
        self.client_id.id == other.client_id.id || (self.client_id.username == other.client_id.username)
    }
}

impl PartialEq for UserInfo {
    fn eq(&self, other: &Self) -> bool {
        self.username == other.username
    }
}

impl Display for Client {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}, ID: {}", self.client_id.username, self.client_id.id)
    }
}

impl Client {
    pub fn generate(username: &String, id: usize) -> Client {
        let username = ClientID { username: username.to_string(), id };
        Client { client_id: username, privilege: Member }
    }

    pub fn username_tag(&self) -> String {
        format!("{}", self.client_id.username).to_string()
    }
}
