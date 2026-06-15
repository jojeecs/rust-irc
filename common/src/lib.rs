//! # Common Library
//!
//! Shared data structures and protocol definitions for the chat server and client.

pub mod room;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use clavis::protocol;
use tokio::sync::mpsc::{Receiver, Sender};
use turso::{Builder, Connection, Row};
use crate::room::room::{Room, RoomStore};

const UID_COLUMN: usize = 0;
const USERNAME_COLUMN: usize = 1;
const PASSWORD_COLUMN: usize = 2;
const ROOM_COLUMN: usize = 3;

/// `Session` holds information regarding a connection between client and server.
///
/// It contains a sender to communicate with the user and basic session metadata.
#[derive(Debug, Clone)]
pub struct Session {
    /// Channel to send events to this specific user.
    pub sender: Arc<Sender<ServerEvent>>,
    /// Metadata about the connection.
    pub session_info: SessionInfo,
}

/// Metadata related to an active user session.
#[derive(Clone, Debug)]
pub struct SessionInfo {
    /// Source IP address of the connected client.
    pub src_ip: IpAddr,
    /// Unique session identifier.
    pub session_id: usize,
    /// Database UID of the connected user.
    pub uid_connected: usize,
}

/// Represents a registered user in the system.
#[derive(Debug)]
pub struct User {
    /// The user's unique username.
    pub username: String,
    /// The user's unique database ID.
    pub user_id: usize,
    pub current_room_name: String,
}

/// Information required for a user to log in.
#[derive(Serialize, Deserialize, Debug, Eq, Clone)]
pub struct LoginInfo {
    pub username: String,
    pub password: String,
}

/// The main server state container.
pub struct Server {
    /// Maps user IDs to their profile and active session.
    pub user_id_map: HashMap<usize, (Arc<User>, Arc<Session>)>,
    /// Maps usernames to their user IDs for quick lookup.
    pub username_map: HashMap<String, usize>,
    /// Storage for all active chat rooms.
    pub room_store: RoomStore,
    /// Global receiver for server-wide events.
    pub receiver: Receiver<ServerEvent>,
    /// Connection to the underlying user database.
    pub db_conn: Connection,
}


protocol! {
    /// Packets sent from the client to the server.
    #[derive(Debug)]
    pub enum ClientPacket {
        /// Request to connect to the server at a given IP.
        ConnectRequest { ip: String },
        /// A public message sent to all users in the current room.
        PublicMessage { contents: String },
        /// A private message sent to a specific user.
        PrivateMessage { to: String, contents: String },
        /// Authentication request with username and password.
        LoginRequestPacket { username: String, password: String },
        /// Sent by the server if a connection attempt is rejected.
        ConnectionRejected { reason: String },
        /// Sent by the server if a connection attempt is accepted.
        ConnectionAccepted,
        /// Sent by the server if authentication fails.
        AuthenticationRejected,
        /// Sent by the server if authentication succeeds.
        AuthenticationAccepted { new_user: bool },
        /// General error packet.
        Error { reason: String },
        RoomUpdate { rooms: Vec<String> },
        RoomChange { new_room_name: String, old_room_name: String },
        /// Notification of client disconnection.
        Disconnect,
    }
}

/// Internal events processed by the server's main loop.
#[derive(Debug)]
pub enum ServerEvent {
    /// Request to log in a user.
    LoginRequest {
        login_details: LoginInfo,
        sender: Arc<Sender<ServerEvent>>,
        ip_src: IpAddr,
    },
    /// Signal that authentication was successful.
    AuthenticationAccept {
        user: Arc<User>,
        new_user: bool,
    },
    /// Signal that authentication failed.
    AuthenticationReject {
        reason: String,
    },
    /// Notification of a received chat message.
    ChatMessageReceive {
        from: usize,
        contents: String,
    },
    /// A generic message.
    Message {
        contents: String,
    },
    /// A private message from an external user.
    DirectMessageExternal {
        to: String,
        from: String,
        contents: String,
    },
    /// Internal representation of a private message.
    PrivateMessage {
        to: String,
        from: usize,
        contents: String,
    },
    /// Signal that a user has disconnected.
    UserDisconnected {
        user: Arc<User>,
    },
    /// A server-side error.
    Error {
        message: String,
    },
    RoomChange {
        new_room_name: String,
        old_room_name: String,
        user_id: usize,
    },
    /// Signal to shut down the server.
    Shutdown,
}

impl Server {
    /// Creates a new Server instance and initializes the database.
    pub async fn new(receiver: Receiver<ServerEvent>) -> Server {
        let db_conn = match Self::init_db().await {
            Some(con) => con,
            None => {
                std::process::exit(0);
            }
        };
        Server {
            user_id_map: HashMap::new(),
            username_map: HashMap::new(),
            room_store: RoomStore::new(),
            receiver,
            db_conn,
        }
    }

    pub async fn find_user_from_username(&self, username: String) -> Option<User> {
        let query = format!("SELECT DISTINCT * FROM users WHERE LOWER(username) LIKE LOWER('{}')", username);
        let rows = match self.run_query(query).await {
            Some(r) => r,
            None => {
                return None;
            }
        };

        if let Some(row) = rows.iter().next() {
            let user_id = match row.get_value(UID_COLUMN) {
                Ok(id) => match id.as_integer() {
                    Some(i) => i.clone(),
                    None => {
                        return None;
                    }
                },
                Err(e) => {
                    eprintln!(
                        "Error getting UID information from row on user {}: {}",
                        username, e
                    );
                    return None;
                }
            };
            let room_name = match row.get_value(ROOM_COLUMN) {
                Ok(room) => match room.as_text() {
                    Some(p) => p.clone(),
                    None => {
                        println!("Error getting room column value");
                        return None;
                    }
                },
                Err(e) => {
                    eprintln!(
                        "Error getting UID information from row on user {}: {}",
                        username, e
                    );
                    return None;
                }
            };
            let username = match row.get_value(USERNAME_COLUMN) {
                Ok(user) => match user.as_text() {
                    Some(u) => u.clone(),
                    None => {
                        println!("Error");
                        return None;
                    }
                },
                Err(e) => {
                    eprintln!("Error: {}", e);
                    return None;
                }
            };
            return Some(User {
                username,
                user_id: user_id as usize,
                current_room_name: room_name,
            });
        } else {
            println!("Unknown error occured ")
        }

        None
    }

    pub async fn get_session_from_username(&self, username: String) -> Option<Arc<Session>> {
        let found = self.user_id_map.get(self.username_map.get(&username)?)?;
        let session = found.clone().1;

        Some(session)
    }
    
    pub async fn get_session_from_uid(&self, uid: usize) -> Option<Arc<Session>> {
        let found = self.user_id_map.get(&uid)?;
        let session = found.clone().1;
        
        Some(session)
    }

    pub async fn user_exists(&self, username: &String) -> bool {
        let command = format!("SELECT DISTINCT * FROM users WHERE LOWER(username) LIKE LOWER('{}')", username);
        let result = self.run_query(command).await.unwrap_or_else(|| Vec::new());

        if let Some(_) = result.iter().next() {
            return true;
        }
        false
    }

    async fn run_query(&self, query: String) -> Option<Vec<Row>> {
        let mut rows = Vec::new();
        let mut result = match self.db_conn.query(query, ()).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Error: {}", e);
                return None;
            }
        };


        loop {
            if let Ok(row) = result.next().await {
                let r = match row {
                    Some(row) => row,
                    None => {
                        break;
                    }
                };
                rows.push(r);
            } else {
                break;
            }
        }
        Some(rows)
    }

    pub async fn verify_credentials(&self, login_info: &LoginInfo) -> bool {
        let query = format!(
            "SELECT DISTINCT * FROM users WHERE LOWER(username) LIKE LOWER('%{}%')",
            login_info.username
        );
        let result = self.run_query(query).await.unwrap_or_else(|| Vec::new());

        if let Some(row) = result.iter().next() {
            match row.get_value(PASSWORD_COLUMN) {
                Ok(value) => match value.as_text() {
                    Some(pass_hash) => return pass_hash.eq(&login_info.password),
                    _ => {}
                },
                Err(e) => {
                    eprintln!(
                        "Error querying db for username {}: {}",
                        login_info.username, e
                    );
                }
            }
        }
        false
    }

    pub async fn create_new_user(&self, username: String, password: String) -> Option<User> {
        let command = "INSERT INTO users (username, password, room) VALUES (?, ?, ?)".to_string();
        if let Err(e) = self
            .db_conn
            .execute(command, (username.trim_ascii(), password.trim_ascii(), "Global".to_string()))
            .await
        {
            eprintln!("Error inserting new user into db: {}", e);
        }

        self.find_user_from_username(username).await
    }

    async fn init_db() -> Option<Connection> {
        let conn;
        let db = match Builder::new_local("./users.db").build().await {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Error building database: {}", e);
                return None;
            }
        };
        conn = match db.connect() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error establishing connection to database: {}", e);
                return None;
            }
        };

        if let Err(e) = conn.execute("CREATE TABLE IF NOT EXISTS users (ID INTEGER PRIMARY KEY AUTOINCREMENT, username VARCHAR(50) UNIQUE, password VARCHAR(100), room VARCHAR(100))", ())
            .await {
            eprintln!("Error initializing db: {}", e);
            return None;
        }

        Some(conn)
    }
}

impl Session {
    pub fn new(
        sender: Arc<Sender<ServerEvent>>,
        src_ip: IpAddr,
        session_id: usize,
        uid_connected: usize,
    ) -> Session {
        let session_info = SessionInfo {
            src_ip,
            session_id,
            uid_connected,
        };

        Session {
            sender,
            session_info,
        }
    }
}

impl User {
    pub fn new(username: String, user_id: usize) -> User {
        User { username, user_id, current_room_name: "Global".to_string() }
    }
}

impl PartialEq for LoginInfo {
    fn eq(&self, other: &Self) -> bool {
        self.username == other.username
    }
}
