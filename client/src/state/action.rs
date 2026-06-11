use common::ClientPacket;
use crate::ui_management::ui_manager::Screen;

pub enum Action {
    ServerConnectionAccepted,
    ServerConnectionFailed,
    LoginAttempt { username: String, password: String },
    SendMessage { contents: String },
    SocketMessage { packet: ClientPacket },
    ScreenCompletion { new_screen_to_show: Screen },
    Exit,
}