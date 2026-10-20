use ratatui::style::Color;

#[derive(Clone)]
pub struct Theme {
    pub key: String,
    pub name: String,
    pub canvas: Color,
    pub panel: Color,
    pub modal: Color,
    pub foreground: Color,
    pub muted: Color,
    pub selection: Color,
    pub added: Color,
    pub removed: Color,
    pub accent: Color,
}

impl Theme {
    pub fn named(name: &str) -> Self {
        match name {
            "one-dark" => Self {
                key: "one-dark".into(),
                name: "One Dark".into(),
                canvas: Color::Rgb(30, 33, 39),
                panel: Color::Rgb(40, 44, 52),
                modal: Color::Rgb(33, 37, 43),
                foreground: Color::Rgb(171, 178, 191),
                muted: Color::Rgb(92, 99, 112),
                selection: Color::Rgb(44, 93, 150),
                added: Color::Rgb(152, 195, 121),
                removed: Color::Rgb(224, 108, 117),
                accent: Color::Rgb(97, 175, 239),
            },
            "one-light" => Self {
                key: "one-light".into(),
                name: "One Light".into(),
                canvas: Color::Rgb(250, 250, 250),
                panel: Color::Rgb(240, 240, 240),
                modal: Color::Rgb(255, 255, 255),
                foreground: Color::Rgb(56, 58, 66),
                muted: Color::Rgb(105, 112, 125),
                selection: Color::Rgb(220, 231, 250),
                added: Color::Rgb(80, 161, 79),
                removed: Color::Rgb(228, 86, 73),
                accent: Color::Rgb(64, 120, 242),
            },
            "contrast" => Self {
                key: "contrast".into(),
                name: "High Contrast".into(),
                canvas: Color::Black,
                panel: Color::Black,
                modal: Color::Black,
                foreground: Color::White,
                muted: Color::Gray,
                selection: Color::Blue,
                added: Color::Green,
                removed: Color::Red,
                accent: Color::Cyan,
            },
            _ => Self {
                key: "terminal".into(),
                name: "Terminal".into(),
                canvas: Color::Reset,
                panel: Color::Reset,
                modal: Color::Reset,
                foreground: Color::Reset,
                muted: Color::DarkGray,
                selection: Color::Blue,
                added: Color::Green,
                removed: Color::Red,
                accent: Color::Cyan,
            },
        }
    }

    pub fn next(&self) -> Self {
        Self::named(match self.key.as_str() {
            "terminal" => "one-dark",
            "one-dark" => "one-light",
            "one-light" => "contrast",
            _ => "terminal",
        })
    }
}
