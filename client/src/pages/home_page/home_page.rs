use tokio::sync::mpsc::UnboundedSender;
use crate::state::action::Action;

pub struct HomePage {
    ui_tx: UnboundedSender<Action>,
}