use std::collections::HashMap;
use std::thread::park;
use crate::event::Event::Crossterm;
use crate::event::UIEvent::{Login, MessageReceived, PostMessage, Quit};
use crate::event::{Event, EventHandler, LoginEvent};
use crate::ui::Screen::{HomePage, LoginPage};
use common::ClientPacket::{
    Disconnect, Handshake, PrivateMessage, PublicMessage,
};
use common::HandshakePacket::{ClientLogin, ClientUsername};
use common::{ClientPacket, LoginInfo};
use crossterm::event;
use crossterm::event::Event::Key;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, ToLine};
use ratatui::widgets::{Block, Paragraph, Widget, Wrap};
use ratatui::{DefaultTerminal, Frame};
use ratatui::style::Color::{Red, White};
use sha3::{Digest, Sha3_256};
use tokio::sync::mpsc::{Receiver, Sender};
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler as EvtHandler;
use crate::ui::Attention::{DmBox, MessageBox, RoomsBox};

#[derive(Debug)]
pub struct App {
    pub running: bool,
    pub screen: Screen,
    pub events: EventHandler,
    pub messages: Vec<String>,
    pub ui_rx: Receiver<ClientPacket>,
    pub socket_tx: Sender<ClientPacket>,
    pub login: LoginInfo,
}

struct Button<'a> {
    label: Line<'a>,
    theme: Theme
}

#[derive(Clone)]
struct Theme {
    text: Color,
    highlight: Color,
    background: Color,
    shadow: Color

}

#[derive(Debug)]
pub enum Screen {
    LoginPage(LoginScreen),
    HomePage(HomeScreen),
}

#[derive(Debug)]
pub struct HomeScreen {
    pub messages: Vec<String>,
    pub input: Input,
    pub private_messages: HashMap<String, Vec<String>>,
    attention: Attention,
}

#[derive(Debug)]
pub enum Attention {
    MessageBox,
    RoomsBox,
    DmBox,
}

#[derive(Debug)]
pub struct LoginScreen {
    username_input: Input,
    password_input: Input,
    confirm_password_input: Input,
    editing_username: bool,
    username_sent: bool,
    new_user: bool,
    password_accepted: bool,
}

impl LoginScreen {
    fn new() -> Self {
        LoginScreen {
            username_input: Input::default(),
            password_input: Input::default(),
            confirm_password_input: Input::default(),
            editing_username: true,
            username_sent: false,
            new_user: false,
            password_accepted: true,
        }
    }
}

impl App {
    pub fn new(ui_rx: Receiver<ClientPacket>, socket_tx: Sender<ClientPacket>) -> Self {
        Self {
            running: true,
            screen:LoginPage(LoginScreen::new()),
            events: EventHandler::new(),
            messages: Vec::new(),
            ui_rx,
            socket_tx,
            login: LoginInfo {
                username: String::new(),
                password: String::new(),
            },
        }
    }

    pub async fn run(mut self, mut terminal: DefaultTerminal) -> color_eyre::Result<()> {
        let mut prompt;

        while self.running {
            match &self.screen {
                LoginPage(login) => {
                    if login.new_user {
                        prompt = "Create New User";
                    } else {
                        prompt = "Login";
                    }
                },
                HomePage(_) => {
                    prompt = "Home";
                },
            }
            terminal.draw(|frame| self.render(frame, prompt))?;

            match self.events.next().await? {
                Crossterm(event) => match event {
                    Key(key_event) if key_event.kind == event::KeyEventKind::Press => {
                        self.screen.handle_input(key_event, &mut self.events)?;
                    }
                    _ => {}
                },
                Event::Ui(Quit) => {
                    self.running = false;
                    let _ = self.socket_tx.send(Disconnect).await;
                }
                Event::Ui(Login(event)) => match event {
                    LoginEvent::Username(username) => {
                        self.login.username = username.clone();
                        if let Err(e) = self
                            .socket_tx
                            .send(Handshake {
                                handshake_packet: ClientUsername { username },
                            })
                            .await
                        {
                            eprintln!("Error occurred sending packet: {}", e);
                        }
                    }
                    LoginEvent::Password(password) => {
                        self.login.password = password.clone();
                        let login_info = LoginInfo {
                            username: self.login.username.clone(),
                            password,
                        };
                        if let Err(e) = self
                            .socket_tx
                            .send(Handshake {
                                handshake_packet: ClientLogin { login_info },
                            })
                            .await
                        {
                            eprintln!("Error: {}", e);
                        };
                    }
                },
                Event::Ui(PostMessage(contents)) => {
                    self.socket_tx.send(raw_msg_to_packet(contents)).await?;
                }
                Event::Ui(MessageReceived(packet)) => match packet {
                    PublicMessage { contents } => {
                        if let HomePage(home) = &mut self.screen {
                            home.messages.push(contents);
                        }
                    },
                    PrivateMessage {to, contents} => {
                        match &mut self.screen {
                            HomePage(home) => {
                                if let Some(list) = home.private_messages.get(&to) {
                                    let mut current_messages = list.clone();
                                    current_messages.push(contents);
                                    home.private_messages.insert(to, current_messages);
                                } else {
                                    home.private_messages.insert(to, vec![contents]);
                                }
                            }
                            _ => {

                            }
                        }
                    },
                    ClientPacket::ConnectionAccepted => {
                        self.screen = HomePage(HomeScreen {
                            messages: self.messages.clone(),
                            input: Default::default(),
                            private_messages: HashMap::new(),
                            attention: MessageBox,
                        });
                    },
                    ClientPacket::ConnectionRejected {..} => {
                        if let LoginPage(login) = &mut self.screen {
                            login.password_accepted = false;
                        }
                    },
                    _ => {}
                },
                _ => {}
            }
            self.tick().await;
        }
        Ok(())
    }

    fn render(&self, frame: &mut Frame, prompt: &str) {
        let [title, rest] =
            Layout::vertical([Constraint::Percentage(3), Constraint::Percentage(97)])
                .areas(frame.area());

        let notifications = Line::raw(prompt).style(Style::default());

        frame.render_widget(notifications, title);
        frame.render_widget(&self.screen, rest);

        if let Some(input) = self.screen.get_input() {
            let width = frame.area().width.max(3) - 3;
            let scroll = input.visual_scroll(width as usize);
            let x = input.visual_cursor().max(scroll) - scroll + 1;

            match &self.screen {
                LoginPage(login) => {
                    let y = if login.editing_username { 1 } else { 4 };
                    frame.set_cursor_position((frame.area().x + x as u16, rest.y + y));
                }
                HomePage(home) => {
                    match home.attention {
                        MessageBox => {
                            frame.set_cursor_position((
                                frame.area().width.div_ceil(4) + x as u16,
                                frame.area().y + frame.area().height - 5,
                            ));
                        } RoomsBox => {
                            frame.set_cursor_position((
                                frame.area().x + 1,
                                frame.area().y + x as u16 + 2,
                            ));
                        }
                        DmBox => {
                            frame.set_cursor_position((
                                (frame.area().width - frame.area().width.div_ceil(4)) + x as u16,
                                frame.area().y + x as u16 + 2,
                            ));
                        }
                    }
                }
            }
        }
    }

    async fn tick(&mut self) {
        match self.ui_rx.try_recv() {
            Ok(msg) => {
                self.events.send(Event::Ui(MessageReceived(msg)));
            },
            Err(_) => {
                return;
            }
        }

    }
}

impl Widget for &HomeScreen {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let home_layout = Layout::horizontal([Constraint::Percentage(25), Constraint::Percentage(50), Constraint::Percentage(25)]);
        let [left, center, right] = area.layout(&home_layout);

        let center_layout = Layout::vertical([Constraint::Min(0), Constraint::Length(6)]);
        let [messages, input] = center.layout(&center_layout);

        let rooms = vec!["Global"];

        let msg_box_style = Style::new().fg(White);
        let in_box_style = Style::default();

        let rooms_box = Paragraph::new(rooms.join("\n"))
            .style(msg_box_style)
            .block(Block::bordered().title("Rooms"));

        rooms_box.render(left, buf);

        let mut scroll = 0;
        if self.messages.len() > messages.height as usize - 3 {
            scroll = (self.messages.len() + 2) - messages.height as usize;
        }

        let all_messages = self.messages.join("\n");
        let message_box = Paragraph::new(all_messages)
            .style(msg_box_style)
            .block(Block::bordered().title("Global Chat"))
            .scroll((scroll.try_into().unwrap(), 0))
            .wrap(Wrap { trim: true });



        let input_box = Paragraph::new(self.input.value())
            .style(in_box_style)
            .block(Block::bordered().title("Message Global Chat"))
            .wrap(Wrap { trim: true });

        message_box.render(messages, buf);
        input_box.render(input, buf);
    }
}

impl Widget for &LoginScreen {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let layout = Layout::vertical([Constraint::Max(3), Constraint::Max(6), Constraint::Max(5)]);
        let [username, password, buttons] = area.layout(&layout);

        let theme = Theme {
            text: Color::Rgb(255, 255, 255),
            background: Color::Rgb(28, 41, 82),
            highlight: Color::Rgb(64, 96, 192),
            shadow: Color::Rgb(32, 48, 96),
        };
        let mut theme_sel = theme.clone();
        theme_sel.background = Red;

        let mut login_button = Button { label: "Login".to_line(), theme: theme.clone() };
        let mut create_button = Button { label: "Create new user".to_line(), theme };

        if self.new_user {
            create_button.theme = theme_sel;
        } else {
            login_button.theme = theme_sel;
        }

        let buttons = Rect::new(buttons.x, buttons.y, 20, buttons.height);


        let button_layout = Layout::vertical([Constraint::Percentage(47), Constraint::Percentage(5), Constraint::Percentage(47)]);
        let [login, _, create] = buttons.layout(&button_layout);

        login_button.render(login, buf);
        create_button.render(create, buf);


        let username_input = Paragraph::new(self.username_input.value())
            .style(Style::default())
            .block(Block::bordered().title("Username"));

        username_input.render(username, buf);

        let pass_style = match self.password_accepted {
            true => {
                Style::new().fg(White)
            },
            false => {
                Style::new().fg(Red)
            }
        };


        let password_layout = Layout::vertical([Constraint::Percentage(47), Constraint::Percentage(5), Constraint::Percentage(47)]);
        let [pass, _, confirm] = password.layout(&password_layout);

        let password_input = Paragraph::new(self.password_input.value())
            .style(pass_style)
            .block(Block::bordered().title("Password"));
        password_input.render(pass, buf);
        if self.new_user {
            let confirm_password_input =  Paragraph::new(self.password_input.value())
                .style(pass_style)
                .block(Block::bordered().title("Confirm password"));
            confirm_password_input.render(confirm, buf);
        }
    }
}

impl Screen {
    fn handle_input(
        &mut self,
        key_event: KeyEvent,
        event_handler: &mut EventHandler,
    ) -> color_eyre::Result<()> {
        match self {
            LoginPage(login) => match key_event.code {
                KeyCode::Esc => {
                    event_handler.send(Event::Ui(Quit));
                }
                KeyCode::Tab => {
                    login.editing_username = !login.editing_username;
                }
                KeyCode::Enter => {
                    if !login.username_sent && !login.username_input.value().is_empty() {
                        event_handler.send(Event::Ui(Login(LoginEvent::Username(
                            login.username_input.value().to_string(),
                        ))));
                        login.editing_username = false;
                        login.username_sent = true;
                    }
                    if !login.password_input.value().is_empty()  {
                        let mut hasher = Sha3_256::new();
                        let pass = login.password_input.value().to_string();
                        hasher.update(pass);
                        let hash = hasher.finalize();

                        let mut password_hash = String::new();

                        for byte in hash {
                            password_hash.push_str(&format!("{:02x}", byte));
                        }

                        event_handler
                            .send(Event::Ui(Login(LoginEvent::Password(password_hash))));
                    }
                },
                KeyCode::Up | KeyCode::Down => {
                    login.new_user = !login.new_user;
                },
                _ => {
                    let event = Key(key_event);
                    if login.editing_username {
                        login.username_input.handle_event(&event);
                    } else {
                        login.password_input.handle_event(&event);
                    }
                }
            },
            HomePage(home) => match key_event.code {
                KeyCode::Esc => {
                    event_handler.send(Event::Ui(Quit));
                },
                KeyCode::Tab => {
                    match home.attention {
                        MessageBox => {
                            home.attention = DmBox;
                        } RoomsBox => {
                            home.attention = MessageBox;
                        }, DmBox => {
                            home.attention = RoomsBox;
                        }
                    }
                },
                KeyCode::Enter => {
                    let contents = home.input.value();
                    if !contents.is_empty() {
                        event_handler.send(Event::Ui(PostMessage(home.input.value_and_reset())));
                    }
                }
                _ => {
                    let event = Key(key_event);
                    home.input.handle_event(&event);
                }
            },
        }

        Ok(())
    }

    fn get_input(&self) -> Option<&Input> {
        match self {
            LoginPage(login) => {
                if login.editing_username {
                    Some(&login.username_input)
                } else {
                    Some(&login.password_input)
                }
            }
            HomePage(home) => Some(&home.input),
        }
    }
}

impl Widget for &Screen {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        match self {
            LoginPage(login) => {
                login.render(area, buf);
            }
            HomePage(home) => {
                home.render(area, buf);
            }
        }
    }
}

impl Widget for Button<'_> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized
    {
        let (background, text, shadow, highlight) = self.colors();
        buf.set_style(area, Style::new().bg(background).fg(text));

        if area.height > 2 {
            buf.set_string(
                area.x,
                area.y,
                "▔".repeat(area.width as usize),
                Style::new().fg(highlight).bg(background),
            );
        }
        // render bottom line if there's enough space
        if area.height > 1 {
            buf.set_string(
                area.x,
                area.y + area.height - 1,
                "▁".repeat(area.width as usize),
                Style::new().fg(shadow).bg(background),
            );
        }

        buf.set_line(
            area.x + (area.width.saturating_sub(self.label.width() as u16)) / 2,
            area.y + (area.height.saturating_sub(1)) / 2,
            &self.label,
            area.width,
        );
    }
}

impl Button<'_> {
    const fn colors(&self) -> (Color, Color, Color, Color) {
        let theme = &self.theme;
        (theme.background, theme.text, theme.shadow, theme.highlight)
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
