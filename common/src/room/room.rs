use std::collections::HashMap;
use rand::{rng, Rng};
use crate::{Session};

pub struct Room {
    pub name: String,
    /// Maps user IDs to their sessions
    pub connections: HashMap<usize, Session>,
    pub messages: Vec<String>,
    pub id: usize,
}

pub struct RoomStore {
    /// Maps room IDs to their respective room
    pub rooms: HashMap<usize, Room>,
}

impl RoomStore {
    pub fn new() -> Self {
        Self {
            rooms: HashMap::new(),
        }
    }

    pub fn add_room_new(&mut self, name: String) -> Option<Room> {
        let new_room = Room::new(name);
        self.rooms.insert(new_room.id, new_room)
    }

    pub fn add_room_existing(&mut self, room: Room) -> Option<Room> {
        self.rooms.insert(room.id, room)
    }

    pub fn remove_room(&mut self, room: Room) -> Option<Room> {
        self.rooms.remove(&room.id)
    }
}

impl Room {
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

    pub fn add_session(&mut self, session: Session) -> Option<Session> {
        if !self.has_session(session.session_info.uid_connected) {
            return self.connections.insert(session.session_info.uid_connected, session);
        }
        None
    }

    pub fn remove_session(&mut self, session: Session) -> Option<Session> {
        self.connections.remove(&session.session_info.uid_connected)
    }

}
