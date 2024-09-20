use iced::highlighter::{self, Highlighter};
use iced::widget::{
    button, column, container, horizontal_space, pick_list, row, text, text_editor, tooltip,
};
use iced::{executor, keyboard, theme, window, Font, Subscription};
use iced::{Application, Command, Element, Length, Settings, Theme};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

// 主函数，程序的入口点。
fn main() -> iced::Result {
    // 运行 Editor 应用程序。
    Editor::run(Settings {
        // 嵌入字体文件，用于文本编辑器中的字体图标。
        fonts: vec![include_bytes!("../fonts/editor-icons.ttf")
            .as_slice()
            .into()],

        window: window::Settings {
            ..Default::default()
        },
        default_font: Font::MONOSPACE,
        ..Default::default()
    })
}

// 定义文本编辑器应用程序的状态。
struct Editor {
    path: Option<PathBuf>,         // 打开文件的路径。
    context: text_editor::Content, // 文本编辑器的内容。
    error: Option<Error>,          // 错误信息。
    theme: highlighter::Theme,     // 代码高亮主题。
    is_dirty: bool,                // 文件是否被修改过。
}

// 定义应用程序可能接收的消息类型。
#[derive(Debug, Clone)]
enum Message {
    Edit(text_editor::Action),                         // 文本编辑器的动作。
    New,                                               // 新建文件。
    Open,                                              // 打开文件。
    FileOpened(Result<(PathBuf, Arc<String>), Error>), // 文件打开结果。
    Save,                                              // 保存文件。
    FileSaved(Result<PathBuf, Error>),                 // 文件保存结果。
    ThemeSelected(highlighter::Theme),                 // 选择的高亮主题。
}

// 为 Editor 结构体实现 iced 的 Application trait。
impl Application for Editor {
    type Message = Message;
    type Executor = executor::Default;
    type Theme = Theme;
    type Flags = ();
    // 创建一个新的 Editor 实例。
    fn new(_flags: Self::Flags) -> (Self, Command<Message>) {
        (
            Self {
                path: None,
                context: text_editor::Content::new(),
                error: None,
                theme: highlighter::Theme::SolarizedDark,
                is_dirty: true,
            },
            Command::perform(load_file(default_file()), Message::FileOpened),
        )
    }
    // 返回应用程序的标题。
    fn title(&self) -> String {
        String::from("A cool Editor !")
    }
    // 根据接收到的消息更新应用程序的状态。
    fn update(&mut self, message: Self::Message) -> Command<Message> {
        match message {
            Message::Edit(action) => {
                self.is_dirty = self.is_dirty || action.is_edit();
                self.error = None;
                self.context.edit(action);
                Command::none()
            }
            Message::New => {
                self.path = None;
                self.context = text_editor::Content::new();
                self.is_dirty = true;
                Command::none()
            }
            Message::Open => Command::perform(pick_file(), Message::FileOpened),
            Message::FileOpened(Ok((path, content))) => {
                self.path = Some(path);
                self.context = text_editor::Content::with(&content);
                self.is_dirty = false;

                Command::none()
            }
            Message::Save => {
                let text = self.context.text();
                self.is_dirty = false;
                Command::perform(save_file(self.path.clone(), text), Message::FileSaved)
            }
            Message::FileSaved(Ok(path)) => {
                self.path = Some(path);
                Command::none()
            }
            Message::FileOpened(Err(error)) => {
                self.error = Some(error);
                Command::none()
            }
            Message::FileSaved(Err(error)) => {
                self.error = Some(error);
                Command::none()
            }
            Message::ThemeSelected(theme) => {
                self.theme = theme;
                Command::none()
            }
        }
    }
    // 创建一个订阅来监听键盘事件。
    fn subscription(&self) -> Subscription<Self::Message> {
        keyboard::on_key_press(|key_code, modifiers| match key_code {
            keyboard::KeyCode::S if modifiers.command() => Some(Message::Save),
            _ => None,
        })
    }
    // 创建应用程序的 UI。
    fn view(&self) -> Element<'_, Message> {
        let controls = row![
            action(new_icon(), "New File", Some(Message::New)),
            action(open_icon(), "Open File", Some(Message::Open)),
            action(
                save_icon(),
                "Save File",
                self.is_dirty.then_some(Message::Save)
            ),
            horizontal_space(Length::Fill),
            pick_list(
                highlighter::Theme::ALL,
                Some(self.theme),
                Message::ThemeSelected
            )
        ]
        .spacing(10);
        let input = text_editor(&self.context)
            .on_edit(Message::Edit)
            .highlight::<Highlighter>(
                highlighter::Settings {
                    theme: self.theme,
                    extension: self
                        .path
                        .as_ref()
                        .and_then(|path| path.extension()?.to_str())
                        .unwrap_or("rs")
                        .to_string(),
                },
                |highlighter, _theme| highlighter.to_format(),
            );

        let status_bar = {
            let status = if let Some(Error::IOFailed(error)) = self.error.as_ref() {
                text(error.to_string())
            } else {
                match self.path.as_deref().and_then(Path::to_str) {
                    Some(path) => text(path).size(14),
                    None => text("New File"),
                }
            };

            let position = {
                let (line, column) = self.context.cursor_position();
                text(format!("{}:{}", line + 1, column + 1))
            };

            row![status, horizontal_space(Length::Fill), position]
        };

        container(column![controls, input, status_bar].spacing(10))
            .padding(10)
            .into()
    }
    // 返回当前应用程序的主题。
    fn theme(&self) -> Theme {
        if self.theme.is_dark() {
            Theme::Dark
        } else {
            Theme::Light
        }
    }
}

// 定义一个函数来创建一个带有图标和标签的按钮，该按钮在被点击时可能会触发一个消息。
fn action<'a>(
    content: Element<'a, Message>, // 按钮中显示的元素，通常是图标。
    label: &str,                   // 按钮的标签，用于鼠标悬停时显示的提示。
    on_press: Option<Message>,     // 可选的点击事件，如果没有则按钮处于禁用状态。
) -> Element<'a, Message> {
    let is_disabled = on_press.is_none(); // 判断按钮是否应该被禁用。
    tooltip(
        button(container(content).width(30).center_x()) // 创建一个包含内容的按钮。
            .on_press_maybe(on_press) // 如果有事件，则设置点击事件。
            .padding([5, 10]) // 设置按钮的内边距。
            .style(if is_disabled {
                // 根据是否禁用来设置按钮的风格。
                theme::Button::Secondary
            } else {
                theme::Button::Primary
            }),
        label,                           // 设置鼠标悬停时的提示文本。
        tooltip::Position::FollowCursor, // 设置提示文本的位置。
    )
    .style(theme::Container::Box) // 设置容器的风格。
    .into() // 转换为 Element。
}

// 定义一个函数来创建一个新的图标元素。
fn new_icon<'a>() -> Element<'a, Message> {
    icon('\u{E800}') // 使用特定的 Unicode 字符作为图标。
}

// 定义一个函数来创建一个保存图标的元素。
fn save_icon<'a>() -> Element<'a, Message> {
    icon('\u{E801}') // 使用特定的 Unicode 字符作为图标。
}

// 定义一个函数来创建一个打开图标的元素。
fn open_icon<'a>() -> Element<'a, Message> {
    icon('\u{F115}') // 使用特定的 Unicode 字符作为图标。
}

// 定义一个函数来创建一个通用的图标元素。
fn icon<'a>(codepoint: char) -> Element<'a, Message> {
    const ICON_FONT: Font = Font::with_name("editor-icons"); // 定义图标字体。

    text(codepoint).font(ICON_FONT).into() // 创建文本元素并应用图标字体。
}

// 定义一个函数来获取默认文件的路径。
fn default_file() -> PathBuf {
    PathBuf::from(format!("{}/src/main.rs", env!("CARGO_MANIFEST_DIR"))) // 使用宏获取默认文件路径。
}

// 定义一个异步函数来打开文件选择对话框并选择文件。
async fn pick_file() -> Result<(PathBuf, Arc<String>), Error> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("Choose a text file")
        .pick_file()
        .await
        .ok_or(Error::DialogClosed)?; // 显示文件选择对话框并处理取消操作。

    load_file(handle.path().to_owned()).await // 加载选择的文件。
}

// 定义一个异步函数来加载文件内容。
async fn load_file(path: PathBuf) -> Result<(PathBuf, Arc<String>), Error> {
    let contexts = tokio::fs::read_to_string(&path)
        .await
        .map(Arc::new)
        .map_err(|error| error.kind())
        .map_err(Error::IOFailed)?; // 读取文件内容并处理可能的错误。

    Ok((path, contexts)) // 返回文件路径和内容。
}
// 定义一个异步函数来保存文件内容。
async fn save_file(path: Option<PathBuf>, text: String) -> Result<PathBuf, Error> {
    let path = if let Some(path) = path {
        path
    } else {
        rfd::AsyncFileDialog::new()
            .set_title("Choose a file name...")
            .save_file()
            .await
            .ok_or(Error::DialogClosed)
            .map(|handle| handle.path().to_owned())? // 显示保存文件对话框并处理取消操作。
    };

    tokio::fs::write(&path, text)
        .await
        .map_err(|error| Error::IOFailed(error.kind()))?; // 写入文件内容并处理可能的错误。

    Ok(path) // 返回文件路径。
}

// 定义错误类型枚举。
#[derive(Debug, Clone)]
enum Error {
    DialogClosed,            // 表示对话框被关闭。
    IOFailed(io::ErrorKind), // 表示输入/输出操作失败。
}
