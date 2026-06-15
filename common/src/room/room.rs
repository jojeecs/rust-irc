//! # Room Management
//!
//! Provides structures and logic for managing chat rooms and their connected sessions.

use std::collections::HashMap;
use std::sync::Arc;
use rand::{rng, Rng};
use crate::{Session};
use crate::ServerEvent::Message;

/// A chat room containing a name, active connections, and message history.
pub struct Room {
    pub name: String,
    /// Maps user IDs to their active sessions in this room.
    pub connections: HashMap<usize, Arc<Session>>,
    /// History of messages sent in this room.
    pub messages: Vec<String>,
    pub id: usize,
}

/// A centralized store for all chat rooms in the server.
pub struct RoomStore {
    /// Maps room IDs to their respective `Room` instance.
    pub rooms: HashMap<usize, Room>,
}

impl RoomStore {
    /// Creates a new, empty `RoomStore`.
    pub fn new() -> Self {
        Self {
            rooms: HashMap::new(),
        }
    }

    pub fn get_room_from_name(&mut self, name: &String) -> Option<&mut Room> {
        let values = self.rooms.values_mut();
        for room in values {
            if room.name.eq(name) {
                return Some(room);
            }
        }
        None
    }

    pub fn add_room_new(&mut self, name: String) -> Option<Room> {
        let new_room = Room::new(name);
        if self.rooms.values().collect::<Vec<_>>().contains(&&new_room) {
            return None;
        }
        self.rooms.insert(new_room.id, new_room)
    }

    pub fn add_room_existing(&mut self, room: Room) -> Option<Room> {
        self.rooms.insert(room.id, room)
    }

    pub fn remove_room(&mut self, room: Room) -> Option<Room> {
        self.rooms.remove(&room.id)
    }
}

impl PartialEq for Room {
    fn eq(&self, other: &Self) -> bool {
        self.name.eq(&other.name) || self.id.eq(&other.id)
    }
}

impl Room {
    /// Creates a new room with the given name and a random ID.
    pub fn new(name: String) -> Self {
        Self {
            name,
            connections: HashMap::new(),
            messages: Vec::new(),
            id: rng().next_u64() as usize
        }
    }

    pub fn create(name: String, id: usize) -> Self {
        let mut me = Self::new(name);
        me.id = id;
        me
    }
    pub fn has_session(&self, user_id: usize) -> bool {
        self.connections.contains_key(&user_id)
    }

    pub fn add_session(&mut self, session: Arc<Session>) -> Option<Arc<Session>> {
        self.connections.insert(session.session_info.uid_connected, session)
    }

    pub fn remove_session(&mut self, session: Arc<Session>) -> Option<Arc<Session>> {
        self.connections.remove(&session.session_info.uid_connected)
    }

    pub async fn new_message(&mut self, contents: String) {
        for session in self.connections.values_mut() {
            if let Err(e) = session.sender.send(Message {contents: contents.clone()}).await {
                eprintln!("Error sending message to uid {}: {}", session.session_info.uid_connected, e);
                continue;
            }
        }
    }
}
