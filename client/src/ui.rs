use crossterm::event;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::event::Event::Key;
use ratatui::buffer::Buffer;
use ratatui::{DefaultTerminal, Frame};
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::widgets::{Block, BorderType, List, Paragraph, Widget};
use tui_input::backend::crossterm::EventHandler as EvtHandler;
use tui_input::Input;
use common::ClientPacket;
use common::ClientPacket::{Disconnect, LoginRequestPacket, PrivateMessage, PublicMessage};
use crate::event::{Event, EventHandler, LoginEvent, UIEvent};
use crate::event::UIEvent::{PostMessage, MessageReceived, Login};
use crate::ui::AppStage::{Chatting, LoginStage};
use crate::ui::LoginSteps::{GotUsername, Init, Quit};

#[derive(Debug)]
pub struct App {
    pub input: Input,
    pub running: bool,
    pub stage: AppStage,
    pub events: EventHandler,
    pub messages: Vec<String>,
    pub ui_rx: UnboundedReceiver<ClientPacket>,
    pub socket_tx: UnboundedSender<ClientPacket>,
}
#[derive(Debug)]
pub enum AppStage {
    LoginStage(LoginSteps),
    Chatting,
}
#[derive(Debug)]
pub enum LoginSteps {
    Init,
    GotUsername,
    GotPassword,
    Quit,
}
impl App {
    pub fn new(ui_rx: UnboundedReceiver<ClientPacket>, socket_tx: UnboundedSender<ClientPacket>) -> Self {
        Self {
            input: Input::default(),
            running: false,
            stage: LoginStage(Init),
            events: EventHandler::new(),
            messages: Vec::new(),
            ui_rx,
            socket_tx,
        }
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> color_eyre::Result<()> {
        self.login(&mut terminal).await?;
        while self.running {
            terminal.draw(|frame|  {
                frame.render_widget(&self, frame.area());
                let width = frame.area().width.max(3) - 3;
                let scroll = self.input.visual_scroll(width as usize);
                let x = self.input.visual_cursor().max(scroll) - scroll + 1;
                frame.set_cursor_position((frame.area().x + x as u16, frame.area().y + 2))
            })?;

            match self.events.next().await? {
                Event::Tick => self.tick().await,
                Event::Crossterm(event) => match event {
                    Key(key_event) => {
                        if key_event.kind == event::KeyEventKind::Press {
                            self.handle_key_events(key_event)?;
                        }
                    }
                    _ => {}
                },
                Event::Ui(event) => match event {
                    UIEvent::Quit => {
                        self.quit()
                    }
                    MessageReceived(msg) => {
                        self.messages.push(self.packet_to_string(msg));
                    },
                    PostMessage(msg) => {
                        let _ = self.socket_tx.send(raw_msg_to_packet(msg));
                    },
                    UIEvent::Login(_) => todo!()
                }
            }
        }

        Ok(())
    }

    async fn login(&mut self, terminal: &mut DefaultTerminal) -> color_eyre::Result<()> {
        let mut username: String = String::new();
        let mut password: String;
        loop {
            match &self.stage {
                LoginStage(_) => {
                    terminal.draw(|frame| self.render_login(frame))?;

                    match self.events.next().await? {
                        Event::Tick => {}
                        Event::Crossterm(event) => match event {
                            Key(key_event) => {
                                if key_event.kind == event::KeyEventKind::Press {
                                    self.handle_key_events(key_event)?;
                                }
                            },
                            _ => {}
                        }
                        Event::Ui(event) => match event {
                            UIEvent::Quit => {
                                self.stage = LoginStage(Quit);
                            }
                            MessageReceived(msg) => {
                                self.messages.push(self.packet_to_string(msg));
                            },
                            PostMessage(msg) => {
                                let _ = self.socket_tx.send(raw_msg_to_packet(msg));
                            },
                            Login(login_event) => {
                                match login_event {
                                    LoginEvent::Username(user) => {
                                        username = user;
                                        self.stage = LoginStage(GotUsername);
                                    }
                                    LoginEvent::Password(pass) => {
                                        password = pass;
                                        let _ = self.socket_tx.send(LoginRequestPacket {username: username.clone(), password});
                                        self.stage = Chatting;
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {
                    break;
                }
            }
            }
        Ok(())
    }

    fn render_login(&mut self, frame: &mut Frame) {
        let [login_area] = Layout::vertical([Constraint::Max(3)]).areas(frame.area());
        let prompt: &str = match &self.stage {
            LoginStage(step) => {
                match step {
                    Init => {"Username"}
                    GotUsername => {"Password"}
                    _ => {
                        ""
                    }
                }
            },
            _ => {
                ""
            }
        };
        self.render_login_input(frame, login_area, prompt);
    }

    fn render_login_input(&self, frame: &mut Frame, area: Rect, prompt: &str) {
        let width = frame.area().width.max(3) - 3;
        let scroll = self.input.visual_scroll(width as usize);
        let style = Style::default();
        let input = Paragraph::new(self.input.value())
            .style(style)
            .scroll((0, scroll as u16))
            .block(Block::bordered().title(prompt));

        frame.render_widget(input, area);

        let x = self.input.visual_cursor().max(scroll) - scroll + 1;
        frame.set_cursor_position((area.x + x as u16, area.y + 1))
    }


    fn packet_to_string(&self, packet: ClientPacket) -> String {

        String::new()
    }

    fn render_messages(&self, area: Rect, buffer: &mut Buffer) {
        let messages = self
            .messages
            .iter()
            .enumerate()
            .map(|(i, message)| format!("{}: {}", i, message));
        let messages = List::new(messages).block(Block::bordered().title("Messages"));
        messages.render(area, buffer);
    }

    fn render_input(&self, area: Rect, buffer: &mut Buffer) {
        let width = area.width.max(3) - 3;
        let scroll = self.input.visual_scroll(width as usize);
        let style = Style::default();
        let input = Paragraph::new(self.input.value())
            .style(style)
            .scroll((0, scroll as u16))
            .block(Block::bordered().title("Input"));
        input.render(area, buffer);
    }
    fn quit(&mut self) {
        self.running = false;
    }
    fn handle_key_events(&mut self, key_event: KeyEvent) -> color_eyre::Result<()> {
        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => self.events.send(UIEvent::Quit),
            KeyCode::Char('c' | 'C') if key_event.modifiers == KeyModifiers::CONTROL => {
                self.events.send(UIEvent::Quit)
            }
            KeyCode::Char(char) => {
                let event = ratatui::crossterm::event::Event::Key(ratatui::crossterm::event::KeyEvent::new(ratatui::crossterm::event::KeyCode::Char(char), ratatui::crossterm::event::KeyModifiers::empty()));
                self.input.handle_event(&event);
            }
            KeyCode::Enter => {
                match &self.stage {
                    LoginStage(step) => {
                        match step {
                            Init => {
                                self.events.send(Login(LoginEvent::Username(self.input.value_and_reset())));
                            },
                            GotUsername => {
                                self.events.send(Login(LoginEvent::Password(self.input.value_and_reset())));
                            }
                            _ => {}
                        }
                    }
                    _ => {
                        self.events.send(PostMessage(self.input.value_and_reset()))
                    }
                }
            },
            _ => {}
        }

        Ok(())
    }

    async fn tick(&mut self) {
        if let Some(msg) = self.ui_rx.recv().await {
            self.events.send(MessageReceived(msg));
        }
    }
}


impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let [header_area, input_area, messages_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(1),
        ]).areas(area);

        let block = Block::bordered()
            .title("{{project-name}}")
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded);

        let text =
            "This is a tui template.\n\
                Press `Esc`, `Ctrl-C` or `q` to stop running.\n\
                Press left and right to increment and decrement the counter respectively.\n\
                Counter";

        let paragraph = Paragraph::new(text)
            .block(block)
            .fg(Color::Cyan)
            .bg(Color::Black)
            .centered();

        paragraph.render(header_area, buf);
        self.render_messages(messages_area, buf);
        self.render_input(input_area, buf);
    }
}

fn raw_msg_to_packet(raw_msg: String) -> ClientPacket {
    if raw_msg.starts_with("/") {
        let mut split = raw_msg.split(" ").collect::<Vec<_>>();
        if let Some(cmd) = split.remove(0).strip_prefix("/") {
            if cmd.eq("pm") {
                let user = split.remove(0);

                let message = split.join(" ");

                return PrivateMessage {
                    to: user.to_string(),
                    contents: message,
                };
            } else if cmd.trim_ascii() == "exit" {
                return Disconnect;
            }
        }
    } else {
        return PublicMessage { contents: raw_msg };
    }

    Disconnect
}