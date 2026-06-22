use std::ops::Div;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Constraint, Position, Rect};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use tokio::sync::mpsc::UnboundedSender;
use tui_input::backend::crossterm::EventHandler as evt;
use crate::components::input::InputField;
use crate::event::Event::ActionEvent;
use crate::event::EventHandler;
use crate::pages::home_page::home_page::HomeField::{MessageInput, RoomSelection, Settings};
use crate::state::action::Action;
use crate::state::action::Action::{RoomChange, SendMessage};
use crate::state::state::{HomeState};
use crate::ui_management::ui_manager::Page;
use crate::components::message_box::{MessageBox as mbox, MessageBox};

pub struct HomePage<'a> {
    pub state: HomeState<'a>,
    pub ui_tx: UnboundedSender<Action>,
    pub message_input: InputField,
    pub message_box: mbox<'a>,
    pub rooms: Vec<String>,
    pub cursor_position: Position,
    pub help_message: String,
    pub modal: Rect,
    pub misc_input: InputField,
}

pub enum HomeField {
    MessageInput,
    RoomSelection,
    Settings,
}

impl<'a> HomePage<'a> {
    pub fn new(ui_tx: UnboundedSender<Action>) -> Self {
        Self {
            state: HomeState::new("Global".to_string()),
            ui_tx,
            message_input: InputField::default(),
            message_box: mbox::new(crossterm::terminal::size().unwrap().0 as usize / 3),
            rooms: Vec::new(),
            cursor_position: Position::ORIGIN,
            help_message: String::default(),
            modal: Rect::default(),
            misc_input: InputField::default(),
        }
    }
}
impl<'a> Page for HomePage<'a> {
    fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let rooms_area = Rect::new(area.x, area.y, area.width.div(3), area.height);
        let center_area = Rect::new(rooms_area.width, area.top() + 5, area.width.div(3), area.height - 5);
        let friends_area = Rect::new(center_area.width, area.y, area.width.div(3), area.height);
        self.render_center(frame, center_area);
        self.render_rooms(frame, rooms_area);
        self.render_friends(frame, friends_area);
        frame.set_cursor_position(self.cursor_position);

        let message_area = Rect::new(center_area.x, area.y, center_area.width, area.height - center_area.height);

        let help_box = Paragraph::new(self.help_message.clone());

        frame.render_widget(help_box, message_area);

        if !self.modal.is_empty() {
            let popup_block = Block::bordered().title("Enter room name");
            let modal_area = area.centered(Constraint::Length(30), Constraint::Length(3));
            let paragraph = Paragraph::new(self.misc_input.value()).block(popup_block);
            frame.render_widget(paragraph, modal_area);
            frame.set_cursor_position((modal_area.x + self.misc_input.display().len() as u16 + 1, modal_area.y + 1));
            
        }
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
                    RoomSelection => {
                        if !self.modal.is_empty() {
                            let _ = self.ui_tx.send(Action::RoomCreationAttempt { name: self.misc_input.input.value_and_reset() });
                            self.modal = Rect::default();
                            return;
                        }
                        let new_room_name = match self.rooms.get(self.state.room_index) {
                            Some(r) => r,
                            _ => {
                                return;
                            }
                        }.to_string();

                        if self.state.current_room_name.eq(&new_room_name) {
                            return;
                        }

                        let _ = self.ui_tx.send(RoomChange {new_room_name: new_room_name.clone(), old_room_name: self.state.current_room_name.clone()});
                        self.state.current_room_name = new_room_name;
                        self.message_box.text.clear();
                    }
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
                match self.state.current_field {
                    MessageInput => {
                        if self.message_box.lines > self.message_box.scroll_amount + 1 {
                            self.message_box.scroll_amount += 1;
                        }
                    }
                    RoomSelection => {
                        if self.state.room_index + 1 < self.rooms.len() {
                            self.cursor_position.y -= 1;
                            self.state.room_index += 1;
                        }
                    }
                    Settings => {}
                }
            },
            KeyCode::Up => {
                match self.state.current_field {
                    MessageInput => {
                        if self.message_box.scroll_amount > 0 {
                            self.message_box.scroll_amount -= 1;
                        }
                    }
                    RoomSelection => {
                        if self.state.room_index > 0 {
                            self.cursor_position.x += 1;
                            self.state.room_index -= 1;
                        }
                    }
                    Settings => {}
                }
            }
            _ => {
                match self.state.current_field {
                    MessageInput => {
                        let event = crossterm::event::Event::Key(event);
                        self.message_input.input.handle_event(&event);
                    },
                    RoomSelection => {
                        if self.modal.is_empty() && let KeyCode::Char(c) = event.code && c == 'e'   {
                            self.modal = Rect::new((self.message_box.width / 2) as u16, (self.message_box.width / 2) as u16, 10, 10);
                        } else {
                            if !self.modal.is_empty() {
                                let event = crossterm::event::Event::Key(event);
                                self.misc_input.input.handle_event(&event);
                            }
                        }
                    }
                    Settings => {

                    }
                }
            }
        }
    }
}

impl<'a> HomePage<'a> {
    fn render_center(&mut self, frame: &mut Frame, area: Rect) {
        let lines = self.message_box.lines;

        let mut messages_box = area.centered_horizontally(Constraint::Percentage(100));
        let mut input_box = area.centered(Constraint::Percentage(100), Constraint::Length(3));

        messages_box.height -= input_box.height;

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
            .block(Block::new().title(self.state.current_room_name.clone()).borders(Borders::ALL).border_type(BorderType::Double)).scroll((scroll_needed as u16, 0));

        let message_input = Paragraph::new(user_input).block(Block::new().borders(Borders::ALL).border_type(BorderType::Plain)).scroll((0, 0));

        frame.render_widget(messages, messages_box);
        frame.render_widget(message_input, input_box);


        if let MessageInput = self.state.current_field {
            let cursor_x_pos = self.message_input.correct_cursor_pos(input_box, new_lines_needed);

            self.cursor_position = Position::new(cursor_x_pos, input_box.y + 1 + new_lines_needed);
        }
    }

    fn render_rooms(&mut self, frame: &mut Frame, area: Rect) {
        let rooms_box = area.centered(Constraint::Percentage(100), Constraint::Percentage(100));

        let rooms = Paragraph::new(self.rooms.join("\n")).block(Block::new().borders(Borders::ALL).border_type(BorderType::Double));

        frame.render_widget(rooms, rooms_box);

        if let RoomSelection = self.state.current_field {
            let cursor_x_pos = rooms_box.x + 1;

            self.cursor_position.x = cursor_x_pos;
            self.cursor_position.y = rooms_box.y + self.state.room_index as u16 + 1;

            self.help_message = String::from("Press <e> to create new room");
        }
    }

    fn render_friends(&mut self, frame: &mut Frame, area: Rect) {

    }
}
