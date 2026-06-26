use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph},
    Frame,
};
use serde::{Deserialize, Deserializer};

use crate::app::{App, Screen};
use crate::mc;

pub const VERSION: &str = "0.5";
const MB: u64 = 1024 * 1024;

// ── Theme system ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct Theme {
    pub name: String,
    #[serde(deserialize_with = "de_color")] pub primary: Color,
    #[serde(deserialize_with = "de_color")] pub primary_light: Color,
    #[serde(deserialize_with = "de_color")] pub primary_dark: Color,
    #[serde(deserialize_with = "de_color")] pub accent: Color,
    #[serde(deserialize_with = "de_color")] pub accent_light: Color,
    #[serde(deserialize_with = "de_color")] pub success: Color,
    #[serde(deserialize_with = "de_color")] pub error: Color,
    #[serde(deserialize_with = "de_color")] pub fg: Color,
    #[serde(deserialize_with = "de_color")] pub fg_dim: Color,
    #[serde(deserialize_with = "de_color")] pub fg_muted: Color,
    #[serde(deserialize_with = "de_color")] pub bg: Color,
    #[serde(deserialize_with = "de_color")] pub bg_light: Color,
}

impl Theme {
    fn border(&self) -> Style          { Style::new().fg(self.primary_dark) }
    fn border_active(&self) -> Style   { Style::new().fg(self.primary) }
    fn title(&self) -> Style           { Style::new().fg(self.primary_light).add_modifier(Modifier::BOLD) }
    fn text(&self) -> Style            { Style::new().fg(self.fg) }
    fn dim(&self) -> Style             { Style::new().fg(self.fg_dim) }
    fn muted(&self) -> Style           { Style::new().fg(self.fg_muted) }
    fn selected(&self) -> Style        { Style::new().bg(self.primary_dark).fg(self.fg).add_modifier(Modifier::BOLD) }
    fn bold_primary(&self) -> Style    { Style::new().fg(self.primary_light).add_modifier(Modifier::BOLD) }

    fn block(&self, border: Style) -> Block<'static> {
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border)
            .style(Style::new().bg(self.bg))
    }
}

#[derive(Deserialize)]
struct ThemeFile { themes: Vec<Theme> }

fn de_color<'de, D: Deserializer<'de>>(d: D) -> Result<Color, D::Error> {
    let [r, g, b]: [u8; 3] = Deserialize::deserialize(d)?;
    Ok(Color::Rgb(r, g, b))
}

pub fn themes() -> &'static [Theme] {
    static THEMES: std::sync::OnceLock<Vec<Theme>> = std::sync::OnceLock::new();
    THEMES.get_or_init(|| {
        toml::from_str::<ThemeFile>(include_str!("../themes/themes.toml"))
            .expect("themes.toml is invalid")
            .themes
    })
}

pub fn current_theme(app: &App) -> &'static Theme {
    &themes()[app.theme_idx % themes().len()]
}

// ── Render dispatch ────────────────────────────────────────────────────────

pub fn render(frame: &mut Frame, app: &App) {
    match app.screen {
        Screen::Main       => render_main(frame, app),
        Screen::Config     => { render_main(frame, app); render_config(frame, app); }
        Screen::Installing => render_installing(frame, app),
        Screen::Auth       => render_auth(frame, app),
        Screen::GameRunning => {}
    }
}

// ── Main screen ────────────────────────────────────────────────────────────

fn header(t: &Theme) -> Block<'static> {
    t.block(t.border())
        .title_alignment(Alignment::Center)
        .title(Line::from(vec![
            Span::styled(" ◆ ", Style::new().fg(t.accent)),
            Span::styled("NEPIX", t.title()),
            Span::styled(" ◆ ", Style::new().fg(t.accent)),
            Span::styled(format!(" v{} ", VERSION), t.muted()),
            Span::styled(format!(" [{}] ", t.name), Style::new().fg(t.accent_light)),
        ]))
}

fn footer(t: &Theme) -> Paragraph<'static> {
    let pairs: &[(Color, &str, &str)] = &[
        (t.primary_light, " ↑↓ ",       "Nav"),
        (t.primary_light, " ←→ ",       "Loader"),
        (t.primary_light, " Ctrl+V ",   "Snapshots"),
        (t.primary_light, " Ctrl+M ",   "Online"),
        (t.primary_light, " Ctrl+P ",   "Config"),
        (t.accent,        " Tab ",      "Theme"),
        (t.success,       " Enter ",    "Launch"),
        (t.accent,        " O ",        "Offline"),
        (t.error,         " Q ",        "Quit"),
    ];
    let mut spans = Vec::new();
    for (i, (color, key, label)) in pairs.iter().enumerate() {
        if i > 0 { spans.push(Span::styled("  │  ", t.muted())); }
        spans.push(Span::styled(*key, Style::new().fg(*color).add_modifier(Modifier::BOLD)));
        spans.push(Span::styled(*label, t.dim()));
    }
    Paragraph::new(Line::from(spans).alignment(Alignment::Center))
        .style(t.dim())
        .block(t.block(t.border()))
}

fn render_main(frame: &mut Frame, app: &App) {
    let t = current_theme(app);
    let area = frame.area();
    let has_error = app.error_message.is_some();
    let mut constraints = vec![Constraint::Length(3), Constraint::Min(1)];
    if has_error { constraints.push(Constraint::Length(3)); }
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
        let err = Paragraph::new(Line::from(vec![
            Span::styled(" ⚠ ", Style::new().fg(t.error).add_modifier(Modifier::BOLD)),
            Span::styled(app.error_message.as_deref().unwrap_or(""), Style::new().fg(t.error)),
        ]))
        .alignment(Alignment::Center)
        .block(t.block(Style::new().fg(t.error)));
        frame.render_widget(err, layout[2]);
    }

    frame.render_widget(footer(t), layout[if has_error { 3 } else { 2 }]);
}

fn render_version_list(frame: &mut Frame, app: &App, area: Rect) {
    let t = current_theme(app);
    let filtered = app.filtered_versions();
    let items: Vec<ListItem> = filtered.iter().enumerate().map(|(i, v)| {
        let is_sel = i == app.selected_version;
        let is_snapshot = v.version_type != "release";
        let (prefix, style) = if is_sel {
            ("▸", t.selected())
        } else if is_snapshot {
            (" ", t.muted())
        } else {
            (" ", t.text())
        };
        let text = if is_snapshot { format!("{} {}  snapshot", prefix, v.id) }
                   else             { format!("{} {}", prefix, v.id) };
        ListItem::new(text).style(style)
    }).collect();

    let subtitle = if app.show_snapshots {
        format!(" {} items ", filtered.len())
    } else {
        format!(" {} releases ", filtered.len())
    };

    let list = List::new(items)
        .block(t.block(t.border()).title(Line::from(vec![
            Span::styled(" Versions ", t.title()),
            Span::styled(subtitle, t.muted()),
        ])))
        .highlight_style(t.selected());

    let mut state = ListState::default().with_selected(Some(app.selected_version));
    frame.render_stateful_widget(list, area, &mut state);
}

fn render_info_panel(frame: &mut Frame, app: &App, area: Rect) {
    let t = current_theme(app);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Length(11), Constraint::Length(3)])
        .split(area);

    let username = if app.config.username.is_empty() { "Not set" } else { &app.config.username };
    let (mode_color, mode_label) = if app.config.online_mode {
        (t.success, "Online (Microsoft)")
    } else {
        (t.accent, "Offline")
    };

    let lbl = Style::new().fg(t.primary_light).add_modifier(Modifier::BOLD);

    let player = Paragraph::new(vec![
        Line::from(vec![Span::raw("  "), Span::styled("Username", lbl), Span::raw("  "), Span::styled(username, t.text())]),
        Line::from(vec![Span::raw("  "), Span::styled("Mode", lbl), Span::raw("      "), Span::styled(mode_label, Style::new().fg(mode_color))]),
        Line::from(""),
        Line::from(vec![
            Span::raw("  "), Span::styled("RAM", lbl), Span::raw("       "),
            Span::styled(&app.config.min_ram, t.text()),
            Span::styled(" / ", t.muted()),
            Span::styled(&app.config.max_ram, t.text()),
        ]),
    ])
    .block(t.block(t.border()).title(Line::from(vec![Span::styled(" Player ", t.title())])));
    frame.render_widget(player, chunks[0]);

    let loader_items: Vec<ListItem> = mc::loader_list().iter().enumerate().map(|(i, l)| {
        let is_sel = i == app.loader_idx;
        let style = if is_sel { t.selected() } else { t.text() };
        let icon = if is_sel { "◉" } else { "○" };
        ListItem::new(format!("  {}  {}", icon, l)).style(style)
    }).collect();

    let loaders = List::new(loader_items).block(
        t.block(t.border()).title(Line::from(vec![
            Span::styled(" Loader ", t.title()),
            Span::styled(format!(" {} ", mc::loader_list()[app.loader_idx]), Style::new().fg(t.accent_light)),
        ]))
    );
    frame.render_widget(loaders, chunks[1]);

    let mod_count = app.config.mods.len();
    let mod_text = if mod_count > 0 { format!("{} loaded", mod_count) } else { "None".into() };
    let mods = Paragraph::new(vec![
        Line::from(vec![
            Span::raw("  "), Span::styled("Mods", lbl), Span::raw("  "),
            Span::styled(mod_text, if mod_count > 0 { t.text() } else { t.muted() }),
        ]),
    ])
    .block(t.block(t.border()));
    frame.render_widget(mods, chunks[2]);
}

// ── Config modal ──────────────────────────────────────────────────────────

pub fn render_config(frame: &mut Frame, app: &App) {
    let t = current_theme(app);
    let area = frame.area();
    let block = t.block(t.border_active()).title(Line::from(vec![
        Span::styled(" ◆ ", Style::new().fg(t.accent)),
        Span::styled("Configuration", t.title()),
        Span::styled(" ◆ ", Style::new().fg(t.accent)),
    ]));

    let modal = centered_rect(75, 75, area);
    frame.render_widget(Clear, modal);
    frame.render_widget(block.clone(), modal);

    let inner = block.inner(modal);
    let chunks = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Length(3), Constraint::Length(3), Constraint::Length(3),
        Constraint::Length(3), Constraint::Length(6), Constraint::Length(3),
        Constraint::Length(3), Constraint::Length(3),
    ]).split(inner);

    let fields = [
        ("Username",  app.edit.username.as_str(),  ""),
        ("Min RAM",   app.edit.min_ram.as_str(),   "e.g. 2G"),
        ("Max RAM",   app.edit.max_ram.as_str(),   "e.g. 4G"),
        ("JVM Args",  app.edit.jvm_args.as_str(),  "comma separated"),
    ];

    for (i, (label, value, hint)) in fields.iter().enumerate() {
        let focused = app.config_focus == i;
        let border = if focused { t.accent } else { t.primary_dark };
        let label_c = if focused { t.accent_light } else { t.primary_light };

        let text = Paragraph::new(Line::from(vec![
            Span::styled(format!("  {label}  "), Style::new().fg(label_c).add_modifier(Modifier::BOLD)),
            Span::styled(*value, t.text()),
            Span::styled(if value.is_empty() { format!("  {hint}") } else { String::new() }, t.muted()),
        ]))
        .block(t.block(Style::new().fg(border)));
        frame.render_widget(text, chunks[i]);
    }

    let mf = app.config_focus == 4;
    let mod_border = if mf { t.accent } else { t.primary_dark };
    let mod_label  = if mf { t.accent_light } else { t.primary_light };
    let mods_text = if app.edit.mods.is_empty() { "No mods configured" } else { &app.edit.mods.join(", ") };

    let mod_para = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("  Mod slug  ", Style::new().fg(mod_label).add_modifier(Modifier::BOLD)),
            Span::styled(&app.edit.mod_input, t.text()),
            Span::styled(if app.edit.mod_input.is_empty() { "  Enter to add" } else { "" }, t.muted()),
        ]),
        Line::from(""),
        Line::from(vec![Span::raw("  "), Span::styled(mods_text, t.dim())]),
    ])
    .block(
        t.block(Style::new().fg(mod_border)).title(Line::from(vec![
            Span::styled(" Mods ", Style::new().fg(mod_label).add_modifier(Modifier::BOLD)),
            Span::styled(" Ctrl+D remove ", t.muted()),
        ]))
    );
    frame.render_widget(mod_para, chunks[4]);

    render_centered_btn(frame, chunks[5], "  Open Game Directory  ", app.config_focus == 5, t);
    render_save_btn(frame, chunks[6], app.config_focus == 6, t);

    let help = Paragraph::new(Line::from(vec![
        Span::styled("  Tab ", t.bold_primary()),   Span::styled("next  ", t.dim()),
        Span::styled("│  ", t.muted()),
        Span::styled(" Enter ", t.bold_primary()),  Span::styled("confirm  ", t.dim()),
        Span::styled("│  ", t.muted()),
        Span::styled(" Esc ", t.bold_primary()),    Span::styled("save & exit", t.dim()),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(help, chunks[7]);
}

fn render_centered_btn(frame: &mut Frame, area: Rect, text: &str, focused: bool, t: &Theme) {
    let border = if focused { t.accent } else { t.primary_dark };
    let fg = if focused { t.accent_light } else { t.fg };
    let btn = Paragraph::new(Line::from(Span::styled(
        text, Style::new().fg(fg).add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center)
    .block(t.block(Style::new().fg(border)));
    frame.render_widget(btn, area);
}

fn render_save_btn(frame: &mut Frame, area: Rect, focused: bool, t: &Theme) {
    let border = if focused { t.success } else { t.primary_dark };
    let btn = Paragraph::new(Line::from(Span::styled(
        "  ✓  Save & Close  ",
        Style::new()
            .fg(if focused { Color::Black } else { t.fg })
            .bg(if focused { t.success } else { Color::Reset })
            .add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center)
    .block(t.block(Style::new().fg(border)));
    frame.render_widget(btn, area);
}

// ── Installing screen ─────────────────────────────────────────────────────

pub fn render_installing(frame: &mut Frame, app: &App) {
    let t = current_theme(app);
    let area = frame.area();
    let layout = Layout::default().direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1), Constraint::Length(3)])
        .split(area);

    frame.render_widget(header(t), layout[0]);

    let center = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Percentage(30), Constraint::Length(3), Constraint::Length(5),
        Constraint::Length(3), Constraint::Length(3), Constraint::Percentage(30),
    ]).split(layout[1]);

    let status = Paragraph::new(Line::from(vec![
        Span::raw("  "),
        Span::styled("⟳ ", t.bold_primary()),
        Span::styled(&app.install_status, t.text()),
    ]))
    .alignment(Alignment::Center)
    .block(t.block(t.border_active()));
    frame.render_widget(status, center[1]);

    let gauge = Gauge::default()
        .block(t.block(t.border_active()))
        .gauge_style(Style::new().fg(t.primary).bg(t.bg_light).add_modifier(Modifier::BOLD))
        .ratio(app.install_progress.clamp(0.0, 1.0))
        .label(format!(" {}% ", (app.install_progress * 100.0) as u64));
    frame.render_widget(gauge, center[2]);

    let bytes = if app.install_total > 0 {
        format!("  {} / {} MB  ", app.install_current / MB, app.install_total / MB)
    } else if app.install_current > 0 {
        format!("  {} MB downloaded  ", app.install_current / MB)
    } else {
        String::new()
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(bytes, t.dim()))).alignment(Alignment::Center),
        center[3],
    );

    let cancel = Paragraph::new(Line::from(vec![
        Span::styled("  Ctrl+C ", Style::new().fg(t.error).add_modifier(Modifier::BOLD)),
        Span::styled("to cancel", t.dim()),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(cancel, center[4]);

    frame.render_widget(footer(t), layout[2]);
}

// ── Auth screen ───────────────────────────────────────────────────────────

fn render_auth(frame: &mut Frame, app: &App) {
    let t = current_theme(app);
    let area = frame.area();
    let layout = Layout::default().direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1), Constraint::Length(3)])
        .split(area);

    frame.render_widget(header(t), layout[0]);
    frame.render_widget(footer_auth(t), layout[2]);

    let center = centered_rect(60, 50, layout[1]);
    let inner = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Length(2), Constraint::Length(1), Constraint::Length(3),
        Constraint::Length(3), Constraint::Length(1), Constraint::Length(1),
    ]).split(center);

    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled("Microsoft Login", t.title())]))
            .alignment(Alignment::Center),
        inner[0],
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::raw("  "), Span::styled(&app.auth_status, t.dim())]))
            .alignment(Alignment::Center),
        inner[1],
    );

    if let (Some(code), Some(url)) = (&app.auth_code, &app.auth_url) {
        let code_text = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("Code: ", t.bold_primary()),
                Span::styled(code, Style::new().fg(t.fg).add_modifier(Modifier::BOLD)),
            ]),
        ])
        .alignment(Alignment::Center)
        .block(t.block(t.border_active()));
        frame.render_widget(code_text, inner[2]);

        let url_text = Paragraph::new(vec![
            Line::from(vec![Span::styled("Open in browser:", t.dim())]),
            Line::from(vec![Span::styled(url, Style::new().fg(t.accent_light))]),
        ])
        .alignment(Alignment::Center)
        .block(t.block(t.border_active()));
        frame.render_widget(url_text, inner[3]);
    }

    let esc = Paragraph::new(Line::from(vec![
        Span::styled("  Esc ", Style::new().fg(t.error).add_modifier(Modifier::BOLD)),
        Span::styled("Cancel", t.dim()),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(esc, inner[5]);
}

fn footer_auth(t: &Theme) -> Paragraph<'static> {
    Paragraph::new(Line::from(vec![
        Span::styled(" Esc ", Style::new().fg(t.error).add_modifier(Modifier::BOLD)),
        Span::styled("Cancel", t.dim()),
    ]))
    .alignment(Alignment::Center)
    .style(t.dim())
    .block(t.block(t.border()))
}

// ── Layout helper ─────────────────────────────────────────────────────────

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ]).split(r);

    Layout::default().direction(Direction::Horizontal).constraints([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ]).split(popup[1])[1]
}
