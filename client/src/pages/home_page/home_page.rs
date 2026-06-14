use std::ops::Div;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use tokio::sync::mpsc::UnboundedSender;
use tui_input::backend::crossterm::EventHandler as evt;
use common::room::room::Room;
use crate::components::input::InputField;
use crate::event::Event::ActionEvent;
use crate::event::EventHandler;
use crate::pages::home_page::home_page::HomeField::{MessageInput, RoomSelection, Settings};
use crate::state::action::Action;
use crate::state::action::Action::SendMessage;
use crate::state::state::{HomeState};
use crate::ui_management::ui_manager::Page;
use crate::components::message::{MessageBox as mbox, MessageBox};

pub struct HomePage<'a> {
    pub state: HomeState<'a>,
    pub ui_tx: UnboundedSender<Action>,
    pub message_input: InputField,
    pub message_box: mbox<'a>,
}

pub enum HomeField {
    MessageInput,
    RoomSelection,
    Settings,
}

impl<'a> HomePage<'a> {
    pub fn new(ui_tx: UnboundedSender<Action>) -> Self {
        Self {
            state: HomeState::new(Room::new("Global".to_string())),
            ui_tx,
            message_input: InputField::default(),
            message_box: mbox::new(crossterm::terminal::size().unwrap().0 as usize / 3)
        }
    }
}
impl<'a> Page for HomePage<'a> {
    fn draw(&self, frame: &mut Frame, area: Rect) {
        let center_area = area.centered(Constraint::Length(area.width.div(3)), Constraint::Percentage(100));
        let rooms_area = Rect::new(area.x, area.y, area.width.div(3), area.height);
        self.render_center(frame, center_area);
    }

    fn handle_event(&mut self, event: KeyEvent, event_handler: &mut EventHandler) {
        match event.code {
            KeyCode::Enter => {
                match self.state.current_field {
                    MessageInput => {
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
                    MessageInput => {
                        Settings
                    }
                    RoomSelection => {
                        MessageInput
                    }
                    Settings => {
                        RoomSelection
                    }
                }
            },
            KeyCode::Down => {
                if self.message_box.lines > self.message_box.scroll_amount {
                    self.message_box.scroll_amount += 1;
                }
            },
            KeyCode::Up => {
                if self.message_box.scroll_amount > 0 {
                    self.message_box.scroll_amount -= 1;
                }
            }
            _ => {
                let event = crossterm::event::Event::Key(event);
                self.message_input.input.handle_event(&event);
            }
        }
    }
}

impl<'a> HomePage<'a> {
    fn render_center(&self, frame: &mut Frame, area: Rect) {
        let lines = self.message_box.lines;

        let messages_box = area.centered(Constraint::Percentage(100), Constraint::Percentage(80));
        let mut input_box = area.centered(Constraint::Percentage(100), Constraint::Length(3));

        input_box.y = messages_box.bottom();

        let mut scroll_needed = 0;

        if lines > messages_box.height as usize - 2 {
            scroll_needed = lines - (messages_box.height as usize - 2);
        }

        scroll_needed += self.message_box.scroll_amount;

        let mut user_input = self.message_input.input.value().to_string();

        let wrapped_data = MessageBox::wrap_msg(area.width as usize - 2, user_input);

        user_input = wrapped_data.0;
        let new_lines_needed = wrapped_data.1 as u16 - 1;

        input_box.height += new_lines_needed;
        input_box.y -= new_lines_needed;

        let messages = Paragraph::new(self.message_box.text.clone())
            .block(Block::new().borders(Borders::ALL).border_type(BorderType::Double)).scroll((scroll_needed as u16, 0));

        let message_input = Paragraph::new(user_input).block(Block::new().borders(Borders::ALL).border_type(BorderType::Plain)).scroll((0, 0));

        frame.render_widget(messages, messages_box);
        frame.render_widget(message_input, input_box);

        let cursor_x_pos = self.message_input.correct_cursor_pos(input_box, new_lines_needed);

        frame.set_cursor_position((cursor_x_pos, input_box.y + 1 + new_lines_needed));
    }

    fn render_rooms(&self, frame: &mut Frame, area: Rect) {

    }
}