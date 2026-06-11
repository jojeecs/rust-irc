use crossterm::event::{KeyCode, KeyEvent};
use crossterm::style::Stylize;
use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::text::{Line, Span, ToLine};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use tokio::sync::mpsc::UnboundedSender;
use tui_input::backend::crossterm::EventHandler as evt;
use common::room::room::Room;
use crate::components::input::InputField;
use crate::event::Event::ActionEvent;
use crate::event::EventHandler;
use crate::pages::home_page::home_page::HomeField::{MessageBox, RoomSelection, Settings};
use crate::state::action::Action;
use crate::state::action::Action::SendMessage;
use crate::state::state::{HomeState};
use crate::ui_management::ui_manager::Page;
use crate::components::message::MessageBox as mbox;

pub struct HomePage {
    pub state: HomeState,
    pub ui_tx: UnboundedSender<Action>,
    pub message_input: InputField,
    pub message_box: mbox,
}

pub enum HomeField {
    MessageBox,
    RoomSelection,
    Settings,
}

impl HomePage {
    pub fn new(ui_tx: UnboundedSender<Action>) -> Self {
        Self {
            state: HomeState::new(Room::new("Global".to_string())),
            ui_tx,
            message_input: InputField::default(),
            message_box: mbox::new(50)
        }
    }
}
impl Page for HomePage {
    fn draw(&self, frame: &mut Frame, area: Rect) {
        let width = area.width.max(3) - 3;

        let messages_box = frame.area().centered(Constraint::Length(50), Constraint::Percentage(80));
        let mut input_box = frame.area().centered(Constraint::Length(50), Constraint::Length(3));

        input_box.y = messages_box.bottom();

        let messages = &self.state.current_room.messages;

        let mut scroll_needed = 0;

        if self.message_box.lines > messages_box.height as usize - 2 {
            scroll_needed = self.message_box.lines - (messages_box.height as usize - 2);
        }

        let mut input_scroll_needed = 0;

        if self.message_input.value().len() > input_box.width as usize - 2 {
            input_scroll_needed = self.message_input.value().len() - (input_box.width as usize - 2);
        }
        
        let messages = Paragraph::new(messages.join("\n"))
            .block(Block::new().borders(Borders::ALL).border_type(BorderType::Double)).scroll((scroll_needed as u16, 0));

        let message_input = Paragraph::new(self.message_input.display()).block(Block::new().borders(Borders::ALL).border_type(BorderType::Plain)).scroll((0, input_scroll_needed as u16));

        frame.render_widget(messages, messages_box);
        frame.render_widget(message_input, input_box);

        let cursor_scroll = self.message_input.input.visual_scroll(width as usize);
        let x = self.message_input.input.visual_cursor().max(cursor_scroll) - cursor_scroll + 1;

        let cursor_x_pos = (input_box.x + x as u16).clamp(input_box.x, (input_box.x + input_box.width) - 2);

        frame.set_cursor_position((cursor_x_pos as u16, input_box.y + 1));
    }

    fn handle_event(&mut self, event: KeyEvent, event_handler: &mut EventHandler) {
        match event.code {
            KeyCode::Enter => {
                match self.state.current_field {
                    MessageBox => {
                        if !self.message_input.is_empty() {
                            event_handler.send(ActionEvent(SendMessage {contents: self.message_input.input.value_and_reset()}));
                        }
                    }
                    RoomSelection => {}
                    Settings => {}
                }
            },
            KeyCode::Tab => {
                self.state.current_field =  match self.state.current_field {
                    MessageBox => {
                        Settings
                    }
                    RoomSelection => {
                        MessageBox
                    }
                    Settings => {
                        RoomSelection
                    }
                }
            },
            _ => {
                let event = crossterm::event::Event::Key(event);
                self.message_input.input.handle_event(&event);
            }
        }
    }
}
