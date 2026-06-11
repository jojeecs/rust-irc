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

/// `ClientSession` holds information regarding a connection between client and server.
///
#[derive(Debug, Clone)]
pub struct Session {
    pub sender: Arc<Sender<ServerEvent>>,
    pub session_info: SessionInfo,
}

#[derive(Clone, Debug)]
pub struct SessionInfo {
    pub src_ip: IpAddr,
    pub session_id: usize,
    pub uid_connected: usize,
}


#[derive(Debug)]
pub struct User {
    pub username: String,
    pub user_id: usize,
}

#[derive(Serialize, Deserialize, Debug, Eq, Clone)]
pub struct LoginInfo {
    pub username: String,
    pub password: String,
}

pub struct Server {
    pub user_id_map: HashMap<usize, (Arc<User>, Arc<Session>)>,
    pub username_map: HashMap<String, usize>,
    pub room_store: RoomStore,
    pub receiver: Receiver<ServerEvent>,
    pub db_conn: Connection,
}


protocol! {
    #[derive(Debug)]
    pub enum ClientPacket {
        ConnectRequest { ip: String },
        PublicMessage { contents: String },
        PrivateMessage { to: String, contents: String },
        LoginRequestPacket { username: String, password: String },
        ConnectionRejected { reason: String },
        ConnectionAccepted,
        AuthenticationRejected,
        AuthenticationAccepted,
        Error { reason: String },
        Disconnect,
    }
}

#[derive(Debug)]
pub enum ServerEvent {
    LoginRequest {
        login_details: LoginInfo,
        sender: Arc<Sender<ServerEvent>>,
        ip_src: IpAddr,
    },
    ConnectionAccept {
        user: Arc<User>,
        session: Arc<Session>,
    },
    ConnectionReject {
        reason: String,
    },
    ChatMessageReceive {
        from: usize,
        contents: String,
    },
    Message {
        contents: String,
    },
    DirectMessageExternal {
        to: String,
        from: String,
        contents: String,
    },
    PrivateMessage {
        to: String,
        from: usize,
        contents: String,
    },
    UserDisconnected {
        user: Arc<User>,
    },
    Error {
        message: String,
    },
    Shutdown,
}

impl Server {
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
        let query = format!("SELECT * FROM users WHERE username = '{}'", username);
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
            return Some(User {
                username,
                user_id: user_id as usize,
            });
        }

        None
    }

    pub async fn get_user_from_uid(&self, uid: usize) -> Option<User> {
        let query = format!("SELECT * FROM user WHERE uid = {}", uid);
        let mut result = match self.db_conn.query(query, ()).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Error querying DB: {}", e);
                return None;
            }
        };

        match result.next().await {
            Ok(row_res) => match row_res {
                Some(row) => {
                    let username = match row.get_value(USERNAME_COLUMN) {
                        Ok(u) => u,
                        Err(e) => {
                            eprintln!("Error parsing DB row: {}", e);
                            return None;
                        }
                    };
                    return match username.as_text() {
                        Some(u) => Some(User::new(u.to_string(), uid)),
                        None => None,
                    };
                }
                None => {}
            },
            Err(e) => {
                eprintln!("Error querying table: {}", e);
                return None;
            }
        }

        None
    }

    async fn name_taken(&self, username: &String) -> bool {
        let command = format!("SELECT DISTINCT * FROM users WHERE LOWER(username) LIKE LOWER('%{}%')", username);
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
        if self.name_taken(&username).await {
            return None;
        }
        let command = "INSERT INTO users (username, password) VALUES (?, ?)".to_string();
        if let Err(e) = self
            .db_conn
            .execute(command, (username.trim_ascii(), password.trim_ascii()))
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

        if let Err(e) = conn.execute("CREATE TABLE IF NOT EXISTS users (ID INTEGER PRIMARY KEY AUTOINCREMENT, username VARCHAR(50) UNIQUE, password VARCHAR(100))", ())
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
        User { username, user_id }
    }
}

impl PartialEq for LoginInfo {
    fn eq(&self, other: &Self) -> bool {
        self.username == other.username
    }
}
