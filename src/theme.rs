use ratatui::style::Color;

pub struct Theme {
    pub name: &'static str,
    pub accent: Color,
    pub accent_flash: Color,
    pub accent_muted: Color,
    pub cursor_fg: Color,
    pub marker_open: Color,
    pub marker_closed: Color,
    pub bg_flash_added: Color,
    pub bg_flash_closed: Color,
    pub bg_flash_reopened: Color,
}

pub const THEMES: [Theme; 3] = [
    Theme {
        name: "Amber",
        accent: Color::Rgb(255, 214, 102),
        accent_flash: Color::Rgb(255, 236, 161),
        accent_muted: Color::Rgb(255, 232, 148),
        cursor_fg: Color::Rgb(16, 18, 22),
        marker_open: Color::Rgb(122, 211, 164),
        marker_closed: Color::Rgb(150, 115, 115),
        bg_flash_added: Color::Rgb(34, 67, 64),
        bg_flash_closed: Color::Rgb(76, 46, 46),
        bg_flash_reopened: Color::Rgb(48, 72, 52),
    },
    Theme {
        name: "Cobalt",
        accent: Color::Rgb(111, 184, 255),
        accent_flash: Color::Rgb(165, 210, 255),
        accent_muted: Color::Rgb(145, 200, 255),
        cursor_fg: Color::Rgb(10, 18, 38),
        marker_open: Color::Rgb(130, 220, 180),
        marker_closed: Color::Rgb(140, 110, 120),
        bg_flash_added: Color::Rgb(25, 55, 80),
        bg_flash_closed: Color::Rgb(70, 35, 45),
        bg_flash_reopened: Color::Rgb(25, 65, 55),
    },
    Theme {
        name: "Rose",
        accent: Color::Rgb(255, 140, 170),
        accent_flash: Color::Rgb(255, 200, 215),
        accent_muted: Color::Rgb(255, 175, 200),
        cursor_fg: Color::Rgb(40, 10, 22),
        marker_open: Color::Rgb(130, 215, 175),
        marker_closed: Color::Rgb(175, 115, 120),
        bg_flash_added: Color::Rgb(35, 62, 52),
        bg_flash_closed: Color::Rgb(78, 38, 48),
        bg_flash_reopened: Color::Rgb(35, 68, 55),
    },
];
