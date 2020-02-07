#![windows_subsystem = "windows"]

use failure::ResultExt;
use futures::AsyncReadExt;
use http_client::HttpClient;
use iced::{
    button, executor, svg, window, Align, Application, Button, Column, Command, Container, Element,
    Length, Row, Settings, Text,
};
use std::path::PathBuf;

mod style;

const THEME: style::Theme = style::Theme::Dark;

pub fn main() {
    HockeyApp::run(Settings {
        window: window::Settings {
            decorations: true,
            resizable: false,
            size: (1024, 768),
        },
        default_font: Some(include_bytes!("../font/Roboto-Regular.ttf")),
    });
}

#[derive(Debug)]
enum HockeyApp {
    Loading,
    Loaded {
        team: Team,
        search: button::State,
    },
    Errored {
        error: Error,
        try_again: button::State,
    },
}

#[derive(Debug, Clone)]
enum Message {
    TeamFound(Result<Team, Error>),
    Search,
}

impl Application for HockeyApp {
    type Message = Message;
    type Executor = executor::Default;

    fn new() -> (HockeyApp, Command<Message>) {
        (
            HockeyApp::Loading,
            Command::perform(Team::search(), Message::TeamFound),
        )
    }

    fn mode(&self) -> window::Mode {
        window::Mode::Windowed
    }

    fn title(&self) -> String {
        let subtitle = match self {
            HockeyApp::Loading => "Loading",
            HockeyApp::Loaded { team, .. } => &team.name,
            HockeyApp::Errored { .. } => "Error occured",
        };

        format!("{} - HockeyApp", subtitle)
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::TeamFound(Ok(team)) => {
                *self = HockeyApp::Loaded {
                    team,
                    search: button::State::new(),
                };

                Command::none()
            }
            Message::TeamFound(Err(error)) => {
                *self = HockeyApp::Errored {
                    error,
                    try_again: button::State::new(),
                };

                Command::none()
            }
            Message::Search => match self {
                HockeyApp::Loading => Command::none(),
                _ => {
                    *self = HockeyApp::Loading;

                    Command::perform(Team::search(), Message::TeamFound)
                }
            },
        }
    }

    fn view(&mut self) -> Element<Message> {
        let content = match self {
            HockeyApp::Loading => Column::new().width(Length::Shrink).push(
                Text::new("Searching for Team...")
                    .width(Length::Shrink)
                    .size(40),
            ),
            HockeyApp::Loaded { team, search } => Column::new()
                .max_width(500)
                .spacing(20)
                .align_items(Align::End)
                .push(team.view())
                .push(
                    button(search, "Keep searching!")
                        .on_press(Message::Search)
                        .style(THEME),
                ),
            HockeyApp::Errored { try_again, .. } => Column::new()
                .width(Length::Shrink)
                .spacing(20)
                .align_items(Align::End)
                .push(
                    Text::new("Whoops! Something went wrong...")
                        .width(Length::Shrink)
                        .size(40),
                )
                .push(
                    button(try_again, "Try again")
                        .on_press(Message::Search)
                        .style(THEME),
                ),
        };

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .style(THEME)
            .into()
    }
}

#[derive(Debug, Clone)]
struct Team {
    number: u32,
    name: String,
    active: bool,
    image: svg::Handle,
}

impl Team {
    fn view(&self) -> Element<Message> {
        Row::new()
            .spacing(20)
            .align_items(Align::Center)
            .push(svg::Svg::new(self.image.clone()))
            .push(
                Column::new()
                    .spacing(20)
                    .push(
                        Row::new()
                            .align_items(Align::Center)
                            .spacing(20)
                            .push(Text::new(&self.name).size(30))
                            .push(
                                Text::new(format!("#{}", self.number))
                                    .width(Length::Shrink)
                                    .size(20)
                                    .color([0.5, 0.5, 0.5]),
                            ),
                    )
                    .push(Text::new(format!("Team is active? {}", self.active))),
            )
            .into()
    }

    async fn search() -> Result<Team, Error> {
        use rand::Rng;

        let stats_client = stats_api::NhlClient::default();
        let mut teams = stats_client.get_teams().await.context("")?;

        let rng_idx = {
            let mut rng = rand::thread_rng();

            rng.gen_range(0, teams.len())
        };

        let team = teams.remove(rng_idx);

        let sprite_path = get_sprite_for_team(team.id).await?;

        let svg = svg::Handle::from_path(sprite_path);

        Ok(Team {
            number: team.id,
            name: team.name,
            active: team.active,
            image: svg,
        })
    }
}

#[derive(Debug, Clone)]
enum Error {
    APIError,
}

impl From<failure::Context<&str>> for Error {
    fn from(error: failure::Context<&str>) -> Error {
        dbg!(&error);

        Error::APIError
    }
}

fn button<'a>(state: &'a mut button::State, text: &str) -> Button<'a, Message> {
    Button::new(state, Text::new(text)).padding(10).style(THEME)
}

async fn get_sprite_for_team(id: u32) -> Result<PathBuf, Error> {
    let mut path = std::env::temp_dir();
    path.push(format!("{}.svg", id));

    if path.is_file() {
        return Ok(path);
    }

    let url = format!(
        "http://www-league.nhlstatic.com/images/logos/teams-current-circle/{}.svg",
        id
    );

    let sprite_client = http_client::native::NativeClient::default();
    let body = http_client::Body::empty();
    let sprite_request = http::Request::get(&url).body(body).context("")?;

    let sprite_request = sprite_client.send(sprite_request);

    let sprite = sprite_request.await.context("")?;
    let mut sprite = sprite.into_body();

    let mut sprite_bytes = vec![];
    sprite.read_to_end(&mut sprite_bytes).await.context("")?;
    std::fs::write(&path, sprite_bytes).context("")?;

    Ok(path)
}
