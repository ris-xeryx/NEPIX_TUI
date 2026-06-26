use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph,
    },
    Frame,
};
use serde::{Deserialize, Deserializer};

use crate::app::{App, Screen};
use crate::mc;

pub const VERSION: &str = "0.4";

#[derive(Deserialize)]
pub struct Theme {
    pub name: String,
    #[serde(deserialize_with = "de_color")]
    pub primary: Color,
    #[serde(deserialize_with = "de_color")]
    pub primary_light: Color,
    #[serde(deserialize_with = "de_color")]
    pub primary_dark: Color,
    #[serde(deserialize_with = "de_color")]
    pub accent: Color,
    #[serde(deserialize_with = "de_color")]
    pub accent_light: Color,
    #[serde(deserialize_with = "de_color")]
    pub success: Color,
    #[serde(deserialize_with = "de_color")]
    pub error: Color,
    #[serde(deserialize_with = "de_color")]
    pub fg: Color,
    #[serde(deserialize_with = "de_color")]
    pub fg_dim: Color,
    #[serde(deserialize_with = "de_color")]
    pub fg_muted: Color,
    #[serde(deserialize_with = "de_color")]
    pub bg: Color,
    #[serde(deserialize_with = "de_color")]
    pub bg_light: Color,
}

#[derive(Deserialize)]
struct ThemeFile {
    themes: Vec<Theme>,
}

fn de_color<'de, D: Deserializer<'de>>(d: D) -> Result<Color, D::Error> {
    let rgb: [u8; 3] = Deserialize::deserialize(d)?;
    Ok(Color::Rgb(rgb[0], rgb[1], rgb[2]))
}

pub fn themes() -> &'static [Theme] {
    static THEMES: std::sync::OnceLock<Vec<Theme>> = std::sync::OnceLock::new();
    THEMES.get_or_init(|| {
        let file: ThemeFile = toml::from_str(include_str!("../themes/themes.toml"))
            .expect("themes.toml is invalid");
        file.themes
    })
}

pub fn current_theme(app: &App) -> &'static Theme {
    let themes = themes();
    &themes[app.theme_idx % themes.len()]
}

fn border_style(t: &Theme) -> Style {
    Style::new().fg(t.primary_dark)
}

fn border_style_active(t: &Theme) -> Style {
    Style::new().fg(t.primary)
}

fn title_style(t: &Theme) -> Style {
    Style::new()
        .fg(t.primary_light)
        .add_modifier(Modifier::BOLD)
}

fn text_style(t: &Theme) -> Style {
    Style::new().fg(t.fg)
}

fn dim_style(t: &Theme) -> Style {
    Style::new().fg(t.fg_dim)
}

fn muted_style(t: &Theme) -> Style {
    Style::new().fg(t.fg_muted)
}

fn selected_style(t: &Theme) -> Style {
    Style::new()
        .bg(t.primary_dark)
        .fg(t.fg)
        .add_modifier(Modifier::BOLD)
}

pub fn render(frame: &mut Frame, app: &App) {
    match app.screen {
        Screen::Main => render_main(frame, app),
        Screen::Config => {
            render_main(frame, app);
            render_config(frame, app);
        }
        Screen::Installing => render_installing(frame, app),
        Screen::Auth => render_auth(frame, app),
        Screen::GameRunning => {}
    }
}

fn header(t: &Theme) -> Block<'static> {
    Block::default()
        .title(Line::from(vec![
            Span::styled(" ◆ ", Style::new().fg(t.accent)),
            Span::styled("NEPIX", title_style(t)),
            Span::styled(" ◆ ", Style::new().fg(t.accent)),
            Span::styled(format!(" v{} ", VERSION), muted_style(t)),
            Span::styled(format!(" [{}] ", t.name), Style::new().fg(t.accent_light)),
        ]))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style(t))
        .style(Style::new().bg(t.bg))
}

fn footer(t: &Theme) -> Paragraph<'static> {
    let help = Line::from(vec![
        Span::styled(" ↑↓ ", Style::new().fg(t.primary_light).add_modifier(Modifier::BOLD)),
        Span::styled("Nav", dim_style(t)),
        Span::styled("  │  ", muted_style(t)),
        Span::styled(" ←→ ", Style::new().fg(t.primary_light).add_modifier(Modifier::BOLD)),
        Span::styled("Loader", dim_style(t)),
        Span::styled("  │  ", muted_style(t)),
        Span::styled(" Ctrl+V ", Style::new().fg(t.primary_light).add_modifier(Modifier::BOLD)),
        Span::styled("Snapshots", dim_style(t)),
        Span::styled("  │  ", muted_style(t)),
        Span::styled(" Ctrl+M ", Style::new().fg(t.primary_light).add_modifier(Modifier::BOLD)),
        Span::styled("Online", dim_style(t)),
        Span::styled("  │  ", muted_style(t)),
        Span::styled(" Ctrl+P ", Style::new().fg(t.primary_light).add_modifier(Modifier::BOLD)),
        Span::styled("Config", dim_style(t)),
        Span::styled("  │  ", muted_style(t)),
        Span::styled(" Tab ", Style::new().fg(t.accent).add_modifier(Modifier::BOLD)),
        Span::styled("Theme", dim_style(t)),
        Span::styled("  │  ", muted_style(t)),
        Span::styled(" Enter ", Style::new().fg(t.success).add_modifier(Modifier::BOLD)),
        Span::styled("Launch", dim_style(t)),
        Span::styled("  │  ", muted_style(t)),
        Span::styled(" O ", Style::new().fg(t.accent).add_modifier(Modifier::BOLD)),
        Span::styled("Offline", dim_style(t)),
        Span::styled("  │  ", muted_style(t)),
        Span::styled(" Q ", Style::new().fg(t.error).add_modifier(Modifier::BOLD)),
        Span::styled("Quit", dim_style(t)),
    ])
    .alignment(Alignment::Center);
    Paragraph::new(help)
        .style(dim_style(t))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(border_style(t))
                .style(Style::new().bg(t.bg)),
        )
}

fn render_main(frame: &mut Frame, app: &App) {
    let t = current_theme(app);
    let area = frame.area();
    let has_error = app.error_message.is_some();
    let mut constraints = vec![Constraint::Length(3), Constraint::Min(1)];
    if has_error {
        constraints.push(Constraint::Length(3));
    }
    constraints.push(Constraint::Length(3));

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    frame.render_widget(header(t), layout[0]);

    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(layout[1]);

    render_version_list(frame, app, main[0]);
    render_info_panel(frame, app, main[1]);

    if has_error {
        let err_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(t.error))
            .style(Style::new().bg(t.bg));
        let err_line = Paragraph::new(Line::from(vec![
            Span::styled(" ⚠ ", Style::new().fg(t.error).add_modifier(Modifier::BOLD)),
            Span::styled(
                app.error_message.as_deref().unwrap_or(""),
                Style::new().fg(t.error),
            ),
        ]))
        .alignment(Alignment::Center)
        .block(err_block);
        frame.render_widget(err_line, layout[2]);
    }

    let footer_idx = if has_error { 3 } else { 2 };
    frame.render_widget(footer(t), layout[footer_idx]);
}

fn render_version_list(frame: &mut Frame, app: &App, area: Rect) {
    let t = current_theme(app);
    let filtered = app.filtered_versions();
    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let is_selected = i == app.selected_version;
            let is_snapshot = v.version_type != "release";

            let (prefix, style) = if is_selected {
                ("▸", selected_style(t))
            } else if is_snapshot {
                (" ", muted_style(t))
            } else {
                (" ", text_style(t))
            };

            let version_text = if is_snapshot {
                format!("{} {}  {}", prefix, v.id, "snapshot")
            } else {
                format!("{} {}", prefix, v.id)
            };

            ListItem::new(version_text).style(style)
        })
        .collect();

    let title = if app.show_snapshots {
        " Versions "
    } else {
        " Versions "
    };

    let subtitle = if app.show_snapshots {
        format!(" {} items ", filtered.len())
    } else {
        format!(" {} releases ", filtered.len())
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(Line::from(vec![
                    Span::styled(title, title_style(t)),
                    Span::styled(subtitle, muted_style(t)),
                ]))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(border_style(t))
                .style(Style::new().bg(t.bg)),
        )
        .highlight_style(selected_style(t));

    let mut state = ListState::default().with_selected(Some(app.selected_version));
    frame.render_stateful_widget(list, area, &mut state);
}

fn render_info_panel(frame: &mut Frame, app: &App, area: Rect) {
    let t = current_theme(app);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),
            Constraint::Length(11),
            Constraint::Length(3),
        ])
        .split(area);

    let username = if app.config.username.is_empty() {
        "Not set".to_string()
    } else {
        app.config.username.clone()
    };

    let auth_mode = if app.config.online_mode {
        (t.success, "Online (Microsoft)")
    } else {
        (t.accent, "Offline")
    };

    let player_info = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("  ", text_style(t)),
            Span::styled("Username", Style::new().fg(t.primary_light).add_modifier(Modifier::BOLD)),
            Span::styled("  ", text_style(t)),
            Span::styled(&username, text_style(t)),
        ]),
        Line::from(vec![
            Span::styled("  ", text_style(t)),
            Span::styled("Mode", Style::new().fg(t.primary_light).add_modifier(Modifier::BOLD)),
            Span::styled("      ", text_style(t)),
            Span::styled(auth_mode.1, Style::new().fg(auth_mode.0)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", text_style(t)),
            Span::styled("RAM", Style::new().fg(t.primary_light).add_modifier(Modifier::BOLD)),
            Span::styled("       ", text_style(t)),
            Span::styled(&app.config.min_ram, text_style(t)),
            Span::styled(" / ", muted_style(t)),
            Span::styled(&app.config.max_ram, text_style(t)),
        ]),
    ])
    .block(
        Block::default()
            .title(Line::from(vec![
                Span::styled(" Player ", title_style(t)),
            ]))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style(t))
            .style(Style::new().bg(t.bg)),
    );
    frame.render_widget(player_info, chunks[0]);

    let loader_items: Vec<ListItem> = mc::loader_list()
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let is_selected = i == app.loader_idx;
            let style = if is_selected {
                selected_style(t)
            } else {
                text_style(t)
            };
            let icon = if is_selected { "◉" } else { "○" };
            ListItem::new(format!("  {}  {}", icon, l)).style(style)
        })
        .collect();

    let loader_list = List::new(loader_items).block(
        Block::default()
            .title(Line::from(vec![
                Span::styled(" Loader ", title_style(t)),
                Span::styled(
                    format!(" {} ", mc::loader_list()[app.loader_idx]),
                    Style::new().fg(t.accent_light),
                ),
            ]))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style(t))
            .style(Style::new().bg(t.bg)),
    );
    frame.render_widget(loader_list, chunks[1]);

    let mod_count = app.edit_mods.len();

    let bottom = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("  ", text_style(t)),
            Span::styled("Mods", Style::new().fg(t.primary_light).add_modifier(Modifier::BOLD)),
            Span::styled("  ", text_style(t)),
            Span::styled(
                if mod_count > 0 {
                    format!("{} loaded", mod_count)
                } else {
                    "None".to_string()
                },
                if mod_count > 0 { text_style(t) } else { muted_style(t) },
            ),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style(t))
            .style(Style::new().bg(t.bg)),
    );
    frame.render_widget(bottom, chunks[2]);
}

pub fn render_config(frame: &mut Frame, app: &App) {
    let t = current_theme(app);
    let area = frame.area();
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" ◆ ", Style::new().fg(t.accent)),
            Span::styled("Configuration", title_style(t)),
            Span::styled(" ◆ ", Style::new().fg(t.accent)),
        ]))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style_active(t))
        .style(Style::new().bg(t.bg));

    let modal = centered_rect(75, 75, area);
    frame.render_widget(Clear, modal);
    frame.render_widget(block.clone(), modal);

    let inner = block.inner(modal);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(6),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(inner);

    let fields = [
        ("Username", &app.edit_username, ""),
        ("Min RAM", &app.edit_min_ram, "e.g. 2G"),
        ("Max RAM", &app.edit_max_ram, "e.g. 4G"),
        ("JVM Args", &app.edit_jvm_args, "comma separated"),
    ];

    for (i, (label, value, hint)) in fields.iter().enumerate() {
        let is_focused = app.config_focus == i;
        let border_color = if is_focused { t.accent } else { t.primary_dark };
        let label_color = if is_focused { t.accent_light } else { t.primary_light };

        let text = Paragraph::new(Line::from(vec![
            Span::styled(format!("  {}  ", label), Style::new().fg(label_color).add_modifier(Modifier::BOLD)),
            Span::styled(value.as_str(), text_style(t)),
            Span::styled(
                if value.is_empty() { format!("  {}", hint) } else { String::new() },
                muted_style(t),
            ),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(border_color)),
        );
        frame.render_widget(text, chunks[i]);
    }

    let mod_fg = if app.config_focus == 4 { t.accent } else { t.primary_dark };
    let mod_label_fg = if app.config_focus == 4 { t.accent_light } else { t.primary_light };
    let mods_text = if app.edit_mods.is_empty() {
        "No mods configured".to_string()
    } else {
        app.edit_mods.join(", ")
    };
    let mod_para = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("  Mod slug  ", Style::new().fg(mod_label_fg).add_modifier(Modifier::BOLD)),
            Span::styled(&app.edit_mod_input, text_style(t)),
            Span::styled(
                if app.edit_mod_input.is_empty() { "  Enter to add" } else { "" },
                muted_style(t),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ", text_style(t)),
            Span::styled(&mods_text, dim_style(t)),
        ]),
    ])
    .block(
        Block::default()
            .title(Line::from(vec![
                Span::styled(" Mods ", Style::new().fg(mod_label_fg).add_modifier(Modifier::BOLD)),
                Span::styled(" Ctrl+D remove ", muted_style(t)),
            ]))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(mod_fg)),
    );
    frame.render_widget(mod_para, chunks[4]);

    let dir_fg = if app.config_focus == 5 { t.accent } else { t.primary_dark };
    let dir_btn = Paragraph::new(Line::from(Span::styled(
        "  Open Game Directory  ",
        Style::new().fg(if app.config_focus == 5 { t.accent_light } else { t.fg }).add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(dir_fg)),
    );
    frame.render_widget(dir_btn, chunks[5]);

    let btn_fg = if app.config_focus == 6 { t.success } else { t.primary_dark };
    let save_btn = Paragraph::new(Line::from(Span::styled(
        "  ✓  Save & Close  ",
        Style::new()
            .fg(if app.config_focus == 6 { Color::Black } else { t.fg })
            .bg(if app.config_focus == 6 { t.success } else { Color::Reset })
            .add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(btn_fg)),
    );
    frame.render_widget(save_btn, chunks[6]);

    let help = Paragraph::new(Line::from(vec![
        Span::styled("  Tab ", Style::new().fg(t.primary_light).add_modifier(Modifier::BOLD)),
        Span::styled("next  ", dim_style(t)),
        Span::styled("│  ", muted_style(t)),
        Span::styled(" Enter ", Style::new().fg(t.primary_light).add_modifier(Modifier::BOLD)),
        Span::styled("confirm  ", dim_style(t)),
        Span::styled("│  ", muted_style(t)),
        Span::styled(" Esc ", Style::new().fg(t.primary_light).add_modifier(Modifier::BOLD)),
        Span::styled("save & exit", dim_style(t)),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(help, chunks[7]);
}

pub fn render_installing(frame: &mut Frame, app: &App) {
    let t = current_theme(app);
    let area = frame.area();
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(area);

    frame.render_widget(header(t), layout[0]);

    let center = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Percentage(30),
        ])
        .split(layout[1]);

    let status_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style_active(t))
        .style(Style::new().bg(t.bg));

    let status = Paragraph::new(Line::from(vec![
        Span::styled("  ", text_style(t)),
        Span::styled("⟳ ", Style::new().fg(t.primary_light).add_modifier(Modifier::BOLD)),
        Span::styled(&app.install_status, text_style(t)),
    ]))
    .alignment(Alignment::Center)
    .block(status_block);
    frame.render_widget(status, center[1]);

    let progress_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style_active(t))
        .style(Style::new().bg(t.bg));

    let gauge = Gauge::default()
        .block(progress_block)
        .gauge_style(
            Style::new()
                .fg(t.primary)
                .bg(t.bg_light)
                .add_modifier(Modifier::BOLD),
        )
        .ratio(app.install_progress.clamp(0.0, 1.0))
        .label(format!(
            " {}% ",
            (app.install_progress * 100.0) as u64
        ));
    frame.render_widget(gauge, center[2]);

    let bytes_info = if app.install_total > 0 {
        format!(
            "  {} / {} MB  ",
            app.install_current / 1024 / 1024,
            app.install_total / 1024 / 1024
        )
    } else if app.install_current > 0 {
        format!("  {} MB downloaded  ", app.install_current / 1024 / 1024)
    } else {
        String::new()
    };

    let bytes_text = Paragraph::new(Line::from(Span::styled(bytes_info, dim_style(t))))
        .alignment(Alignment::Center);
    frame.render_widget(bytes_text, center[3]);

    let cancel = Paragraph::new(Line::from(vec![
        Span::styled("  Ctrl+C ", Style::new().fg(t.error).add_modifier(Modifier::BOLD)),
        Span::styled("to cancel", dim_style(t)),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(cancel, center[4]);

    frame.render_widget(footer(t), layout[2]);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup[1])[1]
}

fn render_auth(frame: &mut Frame, app: &App) {
    let t = current_theme(app);
    let area = frame.area();

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1), Constraint::Length(3)])
        .split(area);

    frame.render_widget(header(t), layout[0]);
    frame.render_widget(footer_auth(t), layout[2]);

    let center = centered_rect(60, 50, layout[1]);

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(center);

    let title = Paragraph::new(Line::from(vec![
        Span::styled("Microsoft Login", title_style(t)),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(title, inner[0]);

    let status = Paragraph::new(Line::from(vec![
        Span::styled("  ", text_style(t)),
        Span::styled(&app.auth_status, dim_style(t)),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(status, inner[1]);

    if let (Some(code), Some(url)) = (&app.auth_code, &app.auth_url) {
        let code_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style_active(t))
            .style(Style::new().bg(t.bg));
        let code_text = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("Code: ", Style::new().fg(t.primary_light).add_modifier(Modifier::BOLD)),
                Span::styled(code, Style::new().fg(t.fg).add_modifier(Modifier::BOLD)),
            ]),
        ])
        .alignment(Alignment::Center)
        .block(code_block);
        frame.render_widget(code_text, inner[2]);

        let url_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style_active(t))
            .style(Style::new().bg(t.bg));
        let url_text = Paragraph::new(vec![
            Line::from(vec![Span::styled("Open in browser:", dim_style(t))]),
            Line::from(vec![Span::styled(url, Style::new().fg(t.accent_light))]),
        ])
        .alignment(Alignment::Center)
        .block(url_block);
        frame.render_widget(url_text, inner[3]);
    }

    let esc_hint = Paragraph::new(Line::from(vec![
        Span::styled("  Esc ", Style::new().fg(t.error).add_modifier(Modifier::BOLD)),
        Span::styled("Cancel", dim_style(t)),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(esc_hint, inner[5]);
}

fn footer_auth(t: &Theme) -> Paragraph<'static> {
    let help = Line::from(vec![
        Span::styled(" Esc ", Style::new().fg(t.error).add_modifier(Modifier::BOLD)),
        Span::styled("Cancel", dim_style(t)),
    ]);
    Paragraph::new(help)
        .alignment(Alignment::Center)
        .style(dim_style(t))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(border_style(t))
                .style(Style::new().bg(t.bg)),
        )
}
