use common::ClientPacket;
use crate::ui_management::ui_manager::Screen;

pub enum Action {
    ServerConnectionAccepted,
    ServerConnectionFailed,
    LoginAttempt { username: String, password: String },
    SendMessage { contents: String },
    SocketMessage { packet: ClientPacket },
    RoomChange { new_room_name: String, old_room_name: String  },
    RoomCreationAttempt { name: String },
    Exit,
}
