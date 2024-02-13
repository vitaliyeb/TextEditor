use std::{env, io};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::{executor, keyboard, theme, Application, Command, Element, Font, Length, Settings, Theme};
use iced::widget::{button, column, container, horizontal_space, row, text, text_editor, tooltip, Text};
use iced::highlighter::{self, Highlighter};

#[derive(Debug, Clone)]
enum Error {
    DialogClosed,
    IOFailed(io::ErrorKind)
}

struct Editor {
    content: text_editor::Content,
    error: Option<Error>,
    path: Option<PathBuf>,
    is_dirty: bool
}

#[derive(Debug, Clone)]
enum Message {
    Edit(text_editor::Action),
    FileOpened(Result<(PathBuf, Arc<String>), Error>),
    Open, 
    New,
    Save,
    FileSave(Result<PathBuf, Error>)
}

impl Application for Editor {
    type Message = Message;
    type Theme = Theme;
    type Executor = executor::Default;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Editor, Command<Message>) {
        (
            Editor {
                path: None,
                content: text_editor::Content::with(""),
                error: None,
                is_dirty: true
            },
            Command::perform(
                load_file(default_file()),
                Message::FileOpened
                )
            )
    }

    fn title(&self) -> String {
        String::from("Dota 2")
    }

    fn update(&mut self, message: Self::Message) -> Command<Message> {
        match message {
            Message::Edit(action) => {
                self.is_dirty = self.is_dirty || action.is_edit();
                self.content.edit(action);

                Command::none()
            },
            Message::Open => Command::perform(pick_file(), Message::FileOpened),
            Message::FileOpened(Ok((path, content))) => {
                self.path = Some(path);
                self.content = text_editor::Content::with(content.as_str());
                self.error = None;

                Command::none()
            },
            Message::FileOpened(Err(error)) => {
                self.is_dirty = false;
                self.error = Some(error);
                Command::none()
            },
            Message::New => {
                self.is_dirty = true;
                self.path = None;
                self.content = text_editor::Content::with("");
                self.error = None;
            
                Command::none()
            },
            Message::FileSave(Ok(path)) => {
                self.path = Some(path);
                self.is_dirty = false;
                Command::none()
            },
            Message::FileSave(Err(error)) => {
                self.error = Some(error);
                Command::none()
            },
            Message::Save => {
                let text = self.content.text();
                Command::perform( save_file(self.path.to_owned(), text), Message::FileSave)
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {

        let controls_bar = {
            let open_file = action(folder_icon(), "Открыть файл",  Some(Message::Open));
            let new_file = action(new_icon(), "Новый файл", Some(Message::New));
            let save_file =  action(save_icon(), "Сохранить файл",  self.is_dirty.then_some(Message::Save));

            row![new_file, open_file, save_file].spacing(10)
        };

        let input = text_editor(&self.content)
        .on_edit(Message::Edit)
        .highlight::<Highlighter>(highlighter::Settings {
            theme: highlighter::Theme::SolarizedDark,
            extension: self
            .path
            .as_ref()
            .and_then(|path| path.extension()?.to_str())
            .unwrap_or("rs")
            .to_string()
        }, |highlight, _theme| {
            highlight.to_format()
        });

        let status_bar = {
            let status = if let Some(Error::IOFailed(error)) = self.error.as_ref() {
                text(error.to_string())
            } else {
                match self.path.as_deref().and_then(Path::to_str) {
                    Some(path) => text(path).size(18),
                    None => text("Новый файл")
                }
            };

            let position: Text = {
                let (line, column) = self.content.cursor_position();
                text(format!("{}:{}", line + 1, column + 1))
            };

            row![status, horizontal_space(Length::Fill), position]
        };    

        container(column![controls_bar, input, status_bar].spacing(10))
            .padding(10)
            .into()
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        keyboard::on_key_press(|key_code, modofiers| match key_code  {
            keyboard::KeyCode::S if modofiers.command() => Some(Message::Save),
            _ => None
        })
    }

}

fn action<'a>(icon: Element<'a, Message>, label: &str, action: Option<Message> ) -> Element<'a, Message> {
    let is_disabled = action.is_none();

    tooltip(
        button(icon)
        .on_press_maybe(action)
        .width(30)
        .style(if is_disabled {
            theme::Button::Secondary
        } else {
            theme::Button::Primary
        }),
        label,
        tooltip::Position::FollowCursor
    )
    .style(theme::Container::Box)
    .into()
}

fn new_icon<'a>() -> Element<'a, Message> {
    icon('\u{E800}')
}

fn folder_icon<'a>() -> Element<'a, Message> {
    icon('\u{F115}')
}

fn save_icon<'a>() -> Element<'a, Message> {
    icon('\u{E801}')
}

fn icon<'a, Message>(codepoint: char) -> Element<'a, Message> {
    const ICON_FONT: Font = Font::with_name("editor-icons");

    text(codepoint).font(ICON_FONT).into()
}

async fn save_file(path: Option<PathBuf>, text: String) -> Result<PathBuf, Error> {
    let path = if let Some(path) = path { path } else {
        rfd::AsyncFileDialog::new()
        .set_title("Choose a file name...")
        .save_file()
        .await
        .ok_or(Error::DialogClosed)
        .map(|handle| handle.path().to_owned())?
    };

    tokio::fs::write(&path, text)
    .await
    .map_err(|err| Error::IOFailed(err.kind()))?;

    Ok(path)
}

fn default_file() -> PathBuf {
    PathBuf::from(format!("{}/src/main.rs", env!("CARGO_MANIFEST_DIR")))
}

async fn load_file(path: PathBuf) -> Result<(PathBuf, Arc<String>), Error> {
    let content = tokio::fs::read_to_string(&path)
    .await
    .map(Arc::new)
    .map_err(|error| Error::IOFailed(error.kind()))?;

    Ok((path, content))
}



async fn pick_file() -> Result<(PathBuf, Arc<String>), Error> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("Выберите файл")
        .pick_file()
        .await.ok_or(Error::DialogClosed)?;

    let path = handle.path(); 

    load_file(path.to_owned()).await
}

pub fn main() -> iced::Result {
    Editor::run(Settings {
        fonts: vec![include_bytes!("../fonts/editor-icons.ttf").as_slice().into()],
        ..Settings::default()
    })
}
