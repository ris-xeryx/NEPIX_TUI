/// Estado interno del launcher y lógica de entrada del teclado.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lighty_auth::{AuthProvider, UserProfile};
use secrecy::ExposeSecret;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Mutex};

use crate::config::Config;
use crate::mc::{self, McEvent, VersionEntry};

#[derive(Debug, PartialEq)]
pub enum Screen {
    Main,
    Config,
    Installing,
    GameRunning,
    Auth,
}

pub enum AuthEvent {
    DeviceCode { url: String, code: String },
    Success { refresh_token: Option<String> },
    Error(String),
}

pub type SharedProfile = Arc<Mutex<Option<UserProfile>>>;

pub struct App {
    pub screen: Screen,
    pub config: Config,
    pub versions: Vec<VersionEntry>,
    pub show_snapshots: bool,
    pub selected_version: usize,
    pub loader_idx: usize,

    pub edit_username: String,
    pub edit_min_ram: String,
    pub edit_max_ram: String,
    pub edit_jvm_args: String,
    pub edit_mods: Vec<String>,
    pub edit_mod_input: String,
    pub config_focus: usize,
    pub config_max_focus: usize,
    pub config_field_dirty: [bool; 6],

    pub theme_idx: usize,

    pub install_status: String,
    pub install_progress: f64,
    pub install_total: u64,
    pub install_current: u64,
    pub install_start: Option<Instant>,
    pub install_has_real_progress: bool,

    pub mc_rx: mpsc::Receiver<McEvent>,
    pub mc_tx: mpsc::Sender<McEvent>,
    pub mc_task: Option<tokio::task::JoinHandle<()>>,

    pub error_message: Option<String>,

    pub auth_code: Option<String>,
    pub auth_url: Option<String>,
    pub auth_status: String,
    pub auth_rx: mpsc::Receiver<AuthEvent>,
    pub auth_tx: mpsc::Sender<AuthEvent>,
    pub auth_profile: SharedProfile,
}

impl App {
    pub fn new(config: Config, versions: Vec<VersionEntry>) -> Self {
        let (mc_tx, mc_rx) = mpsc::channel(256);
        let (auth_tx, auth_rx) = mpsc::channel(32);

        let ver_idx = versions
            .iter()
            .position(|v| v.id == config.last_version)
            .unwrap_or(0);

        let loader_idx = mc::loader_list()
            .iter()
            .position(|l| **l == config.last_loader)
            .unwrap_or(0);

        let edit_mods = config.mods.clone();
        let theme_idx = config.theme;

        Self {
            screen: Screen::Main,
            show_snapshots: config.show_snapshots,
            selected_version: ver_idx,
            loader_idx,
            config,
            versions,
            edit_username: String::new(),
            edit_min_ram: String::new(),
            edit_max_ram: String::new(),
            edit_jvm_args: String::new(),
            edit_mods,
            edit_mod_input: String::new(),
            config_focus: 0,
            config_max_focus: 6,
            config_field_dirty: [false; 6],
            theme_idx,
            install_status: String::new(),
            install_progress: 0.0,
            install_current: 0,
            install_total: 0,
            install_start: None,
            install_has_real_progress: false,
            mc_rx,
            mc_tx,
            mc_task: None,
            error_message: None,
            auth_code: None,
            auth_url: None,
            auth_status: String::new(),
            auth_rx,
            auth_tx,
            auth_profile: Arc::new(Mutex::new(None)),
        }
    }

    pub fn clear_error(&mut self) {
        self.error_message = None;
    }

    pub fn cycle_theme(&mut self) {
        let count = crate::ui::themes().len();
        self.theme_idx = (self.theme_idx + 1) % count;
        self.config.theme = self.theme_idx;
        let _ = self.config.save();
    }

    pub fn filtered_versions(&self) -> Vec<&VersionEntry> {
        self.versions
            .iter()
            .filter(|v| self.show_snapshots || v.version_type == "release")
            .collect()
    }

    pub fn current_version(&self) -> String {
        let list = self.filtered_versions();
        list.get(self.selected_version)
            .map(|v| v.id.clone())
            .unwrap_or_default()
    }

    pub fn start_config(&mut self) {
        self.edit_username = self.config.username.clone();
        self.edit_min_ram = self.config.min_ram.clone();
        self.edit_max_ram = self.config.max_ram.clone();
        self.edit_jvm_args = self.config.jvm_args.clone();
        self.edit_mods = self.config.mods.clone();
        self.edit_mod_input.clear();
        self.config_focus = 0;
        self.config_field_dirty = [false; 6];
        self.screen = Screen::Config;
    }

    pub fn save_config(&mut self) {
        self.config.username = self.edit_username.clone();
        self.config.min_ram = self.edit_min_ram.clone();
        self.config.max_ram = self.edit_max_ram.clone();
        self.config.jvm_args = self.edit_jvm_args.clone();
        self.config.mods = self.edit_mods.clone();
        self.config.show_snapshots = self.show_snapshots;
        let _ = self.config.save();
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        match self.screen {
            Screen::Main => self.handle_main_key(key),
            Screen::Config => self.handle_config_key(key),
            Screen::Installing => self.handle_installing_key(key),
            Screen::GameRunning => true,
            Screen::Auth => self.handle_auth_key(key),
        }
    }

    fn handle_main_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') if key.modifiers == KeyModifiers::NONE => {
                self.config.last_version = self.current_version();
                self.config.last_loader = mc::loader_list()[self.loader_idx].to_string();
                let _ = self.config.save();
                return true;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let count = self.filtered_versions().len();
                if count > 0 {
                    self.selected_version = self.selected_version.saturating_sub(1);
                }
                self.clear_error();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let count = self.filtered_versions().len();
                if count > 0 {
                    self.selected_version = (self.selected_version + 1).min(count - 1);
                }
                self.clear_error();
            }
            KeyCode::Left => {
                self.loader_idx = self.loader_idx.saturating_sub(1);
                self.clear_error();
            }
            KeyCode::Right => {
                let max = mc::loader_list().len() - 1;
                self.loader_idx = (self.loader_idx + 1).min(max);
                self.clear_error();
            }
            KeyCode::Char('p') | KeyCode::Char('P')
                if key.modifiers == KeyModifiers::CONTROL =>
            {
                self.start_config();
            }
            KeyCode::Char('v') | KeyCode::Char('V')
                if key.modifiers == KeyModifiers::CONTROL =>
            {
                self.show_snapshots = !self.show_snapshots;
                self.selected_version = 0;
            }
            KeyCode::Char('m') | KeyCode::Char('M')
                if key.modifiers == KeyModifiers::CONTROL =>
            {
                self.config.online_mode = !self.config.online_mode;
                let _ = self.config.save();
            }
            KeyCode::Enter => {
                if self.config.online_mode {
                    self.start_auth();
                } else {
                    self.launch_minecraft();
                }
            }
            KeyCode::Tab => {
                self.cycle_theme();
            }
            KeyCode::Char('o') | KeyCode::Char('O') if key.modifiers == KeyModifiers::NONE => {
                self.launch_minecraft_offline();
            }
            _ => {}
        }
        false
    }

    fn start_auth(&mut self) {
        self.screen = Screen::Auth;
        self.auth_status = "Connecting to Microsoft...".into();
        self.auth_code = None;
        self.auth_url = None;

        let refresh_token = self.config.msa_refresh_token.clone();
        let tx = self.auth_tx.clone();
        let tx_success = self.auth_tx.clone();
        let profile_arc = self.auth_profile.clone();

        tokio::spawn(async move {
            let result = mc::authenticate_microsoft(
                refresh_token.as_deref(),
                move |code, url| {
                    let tx = tx.clone();
                    let code = code.to_string();
                    let url = url.to_string();
                    tokio::spawn(async move {
                        let _ = tx.send(AuthEvent::DeviceCode { url, code }).await;
                    });
                },
            ).await;

            match result {
                Ok(profile) => {
                    let rt = match &profile.provider {
                        AuthProvider::Microsoft { refresh_token, .. } => {
                            refresh_token.as_ref().map(|rt| rt.expose_secret().to_string())
                        }
                        _ => None,
                    };
                    *profile_arc.lock().await = Some(profile);
                    let _ = tx_success.send(AuthEvent::Success { refresh_token: rt }).await;
                }
                Err(e) => {
                    let _ = tx_success.send(AuthEvent::Error(format!("{e}"))).await;
                }
            }
        });
    }

    pub fn handle_auth_event(&mut self, event: AuthEvent) {
        match event {
            AuthEvent::DeviceCode { url, code } => {
                self.auth_code = Some(code);
                self.auth_url = Some(url);
                self.auth_status = "Login with Microsoft".into();
            }
            AuthEvent::Success { refresh_token } => {
                if let Some(rt) = refresh_token {
                    self.config.msa_refresh_token = Some(rt);
                    let _ = self.config.save();
                }
                self.auth_status = "Authenticated! Launching...".into();
                self.launch_minecraft();
            }
            AuthEvent::Error(e) => {
                self.auth_status = format!("Auth error: {e}");
            }
        }
    }

    fn handle_auth_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => {
                self.screen = Screen::Main;
                self.auth_status.clear();
                self.auth_code = None;
                self.auth_url = None;
            }
            _ => {}
        }
        false
    }

    pub fn launch_minecraft(&mut self) {
        let version = self.current_version();
        let loader = mc::loader_list()[self.loader_idx].to_string();
        let min_ram = self.config.min_ram.clone();
        let max_ram = self.config.max_ram.clone();
        let jvm_args: Vec<String> = self
            .config
            .jvm_args
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        let username = self.config.username.clone();
        let online = self.config.online_mode;

        let tx = self.mc_tx.clone();
        let mods = self.config.mods.clone();
        let profile_arc = self.auth_profile.clone();

        self.mc_task = Some(tokio::spawn(async move {
            let p = if online {
                let guard = profile_arc.lock().await;
                match guard.as_ref() {
                    Some(profile) => profile.clone(),
                    None => {
                        let _ = tx.send(McEvent::Error(
                            "Online mode is on but no Microsoft session found. Press O for offline launch.".into()
                        )).await;
                        return;
                    }
                }
            } else {
                match mc::authenticate_offline(&username).await {
                    Ok(profile) => profile,
                    Err(e) => {
                        let _ = tx.send(McEvent::Error(format!("Auth failed: {e}"))).await;
                        return;
                    }
                }
            };
            mc::launch(&p, version, loader, min_ram, max_ram, &jvm_args, &mods, tx).await;
        }));

        self.install_status = "Starting...".into();
        self.install_progress = 0.0;
        self.install_current = 0;
        self.install_total = 0;
        self.install_start = Some(Instant::now());
        self.install_has_real_progress = false;
        self.screen = Screen::Installing;
    }

    pub fn launch_minecraft_offline(&mut self) {
        let version = self.current_version();
        let loader = mc::loader_list()[self.loader_idx].to_string();
        let min_ram = self.config.min_ram.clone();
        let max_ram = self.config.max_ram.clone();
        let jvm_args: Vec<String> = self
            .config
            .jvm_args
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        let username = self.config.username.clone();

        let tx = self.mc_tx.clone();
        let mods = self.config.mods.clone();

        self.mc_task = Some(tokio::spawn(async move {
            let p = match mc::authenticate_offline(&username).await {
                Ok(profile) => profile,
                Err(e) => {
                    let _ = tx.send(McEvent::Error(format!("Auth failed: {e}"))).await;
                    return;
                }
            };
            mc::launch(&p, version, loader, min_ram, max_ram, &jvm_args, &mods, tx).await;
        }));

        self.install_status = "Starting... (offline)".into();
        self.install_progress = 0.0;
        self.install_current = 0;
        self.install_total = 0;
        self.install_start = Some(Instant::now());
        self.install_has_real_progress = false;
        self.screen = Screen::Installing;
    }

    pub fn handle_mc_event(&mut self, event: McEvent) {
        crate::log::info("APP", &format!("handle_mc_event: {event:?}"));
        match event {
            McEvent::Status(s) => {
                self.install_status = s;
            }
            McEvent::Progress { current, total } => {
                if total > 0 {
                    self.install_total = total;
                    self.install_has_real_progress = true;
                    self.install_progress = current as f64 / total as f64;
                } else if self.install_total > 0 {
                    self.install_progress = current as f64 / self.install_total as f64;
                }
                self.install_current = current;
            }
            McEvent::Launched { .. } => {
                self.install_progress = 1.0;
                self.install_status = "Launching...".into();
                self.screen = Screen::GameRunning;
            }
            McEvent::ProcessOutput(_) => {}
            McEvent::ProcessExited { .. } => {
                self.config.last_version = self.current_version();
                let _ = self.config.save();
                self.screen = Screen::Main;
            }
            McEvent::Error(e) => {
                self.install_progress = 1.0;
                self.error_message = Some(e);
                self.screen = Screen::Main;
            }
            McEvent::Done => {
                self.install_progress = 1.0;
                if self.screen == Screen::Installing {
                    self.screen = Screen::Main;
                }
            }
        }
    }

    pub fn advance_fake_progress(&mut self) {
        if self.install_has_real_progress {
            return;
        }
        if let Some(start) = self.install_start {
            let elapsed = start.elapsed().as_secs_f64();
            self.install_progress = (1.0 - (-elapsed / 8.0).exp()) * 0.85;
        }
    }

    fn handle_config_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => {
                self.save_config();
                self.config.last_version = self.current_version();
                self.config.last_loader = mc::loader_list()[self.loader_idx].to_string();
                let _ = self.config.save();
                self.edit_mod_input.clear();
                self.screen = Screen::Main;
            }
            KeyCode::Tab | KeyCode::Down => {
                self.config_focus = (self.config_focus + 1) % (self.config_max_focus + 1);
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.config_focus = if self.config_focus == 0 {
                    self.config_max_focus
                } else {
                    self.config_focus - 1
                };
            }
            KeyCode::Enter => match self.config_focus {
                0 | 1 | 2 | 3 => {
                    self.config_focus = (self.config_focus + 1) % (self.config_max_focus + 1);
                }
                4 => {
                    if !self.edit_mod_input.is_empty() {
                        self.edit_mods.push(self.edit_mod_input.clone());
                        self.edit_mod_input.clear();
                    }
                }
                5 => {
                    let dir = lighty_launcher::core::AppState::data_dir();
                    let _ = open::that(dir);
                }
                6 => {
                    self.save_config();
                    self.config.last_version = self.current_version();
                    self.config.last_loader = mc::loader_list()[self.loader_idx].to_string();
                    let _ = self.config.save();
                    self.screen = Screen::Main;
                }
                _ => {}
            },
            KeyCode::Char('d') | KeyCode::Char('D')
                if key.modifiers == KeyModifiers::CONTROL =>
            {
                if self.config_focus == 4 && !self.edit_mods.is_empty() {
                    self.edit_mods.pop();
                }
            }
            KeyCode::Char(c) => match self.config_focus {
                0 => {
                    if !self.config_field_dirty[0] { self.edit_username.clear(); self.config_field_dirty[0] = true; }
                    self.edit_username.push(c);
                }
                1 => {
                    if !self.config_field_dirty[1] { self.edit_min_ram.clear(); self.config_field_dirty[1] = true; }
                    self.edit_min_ram.push(c);
                }
                2 => {
                    if !self.config_field_dirty[2] { self.edit_max_ram.clear(); self.config_field_dirty[2] = true; }
                    self.edit_max_ram.push(c);
                }
                3 => {
                    if !self.config_field_dirty[3] { self.edit_jvm_args.clear(); self.config_field_dirty[3] = true; }
                    self.edit_jvm_args.push(c);
                }
                4 => {
                    if !self.config_field_dirty[4] { self.edit_mod_input.clear(); self.config_field_dirty[4] = true; }
                    self.edit_mod_input.push(c);
                }
                _ => {}
            },
            KeyCode::Backspace => match self.config_focus {
                0 => { self.edit_username.pop(); }
                1 => { self.edit_min_ram.pop(); }
                2 => { self.edit_max_ram.pop(); }
                3 => { self.edit_jvm_args.pop(); }
                4 => { self.edit_mod_input.pop(); }
                _ => {}
            },
            _ => {}
        }
        false
    }

    fn handle_installing_key(&mut self, key: KeyEvent) -> bool {
        if let KeyCode::Char('c') | KeyCode::Char('C') = key.code {
            if key.modifiers == KeyModifiers::CONTROL {
                if let Some(task) = self.mc_task.take() {
                    task.abort();
                }
                self.install_status.clear();
                self.install_progress = 0.0;
                self.install_current = 0;
                self.install_total = 0;
                self.screen = Screen::Main;
            }
        }
        false
    }
}
