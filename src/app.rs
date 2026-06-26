/// Estado del launcher y lógica de teclado.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lighty_auth::AuthProvider;
use secrecy::ExposeSecret;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Mutex};

use crate::config::Config;
use crate::mc::{self, McEvent, VersionEntry};

pub type SharedProfile = Arc<Mutex<Option<lighty_auth::UserProfile>>>;

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

pub struct App {
    pub screen: Screen,
    pub config: Config,
    pub versions: Vec<VersionEntry>,
    pub show_snapshots: bool,
    pub selected_version: usize,
    pub loader_idx: usize,
    pub theme_idx: usize,

    pub edit: ConfigEdit,
    pub config_focus: usize,

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

pub struct ConfigEdit {
    pub username: String,
    pub min_ram: String,
    pub max_ram: String,
    pub jvm_args: String,
    pub mods: Vec<String>,
    pub mod_input: String,
    pub field_dirty: [bool; 5],
}

const CONFIG_MAX_FOCUS: usize = 6;

impl App {
    pub fn new(config: Config, versions: Vec<VersionEntry>) -> Self {
        let (mc_tx, mc_rx) = mpsc::channel(256);
        let (auth_tx, auth_rx) = mpsc::channel(32);

        let selected_version = versions
            .iter()
            .position(|v| v.id == config.last_version)
            .unwrap_or(0);

        let loader_idx = mc::loader_list()
            .iter()
            .position(|l| **l == config.last_loader)
            .unwrap_or(0);

        let theme_idx = config.theme;

        Self {
            screen: Screen::Main,
            show_snapshots: config.show_snapshots,
            selected_version,
            loader_idx,
            theme_idx,
            config,
            versions,
            edit: ConfigEdit {
                username: String::new(),
                min_ram: String::new(),
                max_ram: String::new(),
                jvm_args: String::new(),
                mods: Vec::new(),
                mod_input: String::new(),
                field_dirty: [false; 5],
            },
            config_focus: 0,
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
        self.filtered_versions()
            .get(self.selected_version)
            .map(|v| v.id.clone())
            .unwrap_or_default()
    }

    pub fn start_config(&mut self) {
        let c = &self.config;
        self.edit = ConfigEdit {
            username: c.username.clone(),
            min_ram: c.min_ram.clone(),
            max_ram: c.max_ram.clone(),
            jvm_args: c.jvm_args.clone(),
            mods: c.mods.clone(),
            mod_input: String::new(),
            field_dirty: [false; 5],
        };
        self.config_focus = 0;
        self.screen = Screen::Config;
    }

    fn save_config(&mut self) {
        self.config.username = self.edit.username.clone();
        self.config.min_ram = self.edit.min_ram.clone();
        self.config.max_ram = self.edit.max_ram.clone();
        self.config.jvm_args = self.edit.jvm_args.clone();
        self.config.mods = self.edit.mods.clone();
        self.config.show_snapshots = self.show_snapshots;
        let _ = self.config.save();
    }

    fn persist_state(&mut self) {
        self.config.last_version = self.current_version();
        self.config.last_loader = mc::loader_list()[self.loader_idx].to_string();
        let _ = self.config.save();
    }

    fn parse_jvm_args(&self) -> Vec<String> {
        self.config
            .jvm_args
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    fn reset_install_state(&mut self, status: &str) {
        self.install_status = status.into();
        self.install_progress = 0.0;
        self.install_current = 0;
        self.install_total = 0;
        self.install_start = Some(Instant::now());
        self.install_has_real_progress = false;
        self.screen = Screen::Installing;
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
            KeyCode::Char('q' | 'Q') if key.modifiers == KeyModifiers::NONE => {
                self.persist_state();
                return true;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !self.filtered_versions().is_empty() {
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
            KeyCode::Char('p' | 'P') if key.modifiers == KeyModifiers::CONTROL => self.start_config(),
            KeyCode::Char('v' | 'V') if key.modifiers == KeyModifiers::CONTROL => {
                self.show_snapshots = !self.show_snapshots;
                self.selected_version = 0;
            }
            KeyCode::Char('m' | 'M') if key.modifiers == KeyModifiers::CONTROL => {
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
            KeyCode::Tab => self.cycle_theme(),
            KeyCode::Char('o' | 'O') if key.modifiers == KeyModifiers::NONE => {
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
            )
            .await;

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
        if key.code == KeyCode::Esc {
            self.screen = Screen::Main;
            self.auth_status.clear();
            self.auth_code = None;
            self.auth_url = None;
        }
        false
    }

    pub fn launch_minecraft(&mut self) {
        let version = self.current_version();
        let loader = mc::loader_list()[self.loader_idx].to_string();
        let min_ram = self.config.min_ram.clone();
        let max_ram = self.config.max_ram.clone();
        let jvm_args = self.parse_jvm_args();
        let username = self.config.username.clone();
        let online = self.config.online_mode;
        let mods = self.config.mods.clone();
        let tx = self.mc_tx.clone();
        let profile_arc = self.auth_profile.clone();

        self.mc_task = Some(tokio::spawn(async move {
            let p = if online {
                let guard = profile_arc.lock().await;
                match guard.as_ref() {
                    Some(profile) => profile.clone(),
                    None => {
                        let _ = tx.send(McEvent::Error(
                            "Online mode is on but no Microsoft session. Press O for offline.".into(),
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

        self.reset_install_state("Starting...");
    }

    pub fn launch_minecraft_offline(&mut self) {
        let version = self.current_version();
        let loader = mc::loader_list()[self.loader_idx].to_string();
        let min_ram = self.config.min_ram.clone();
        let max_ram = self.config.max_ram.clone();
        let jvm_args = self.parse_jvm_args();
        let username = self.config.username.clone();
        let mods = self.config.mods.clone();
        let tx = self.mc_tx.clone();

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

        self.reset_install_state("Starting... (offline)");
    }

    pub fn handle_mc_event(&mut self, event: McEvent) {
        crate::log::info("APP", &format!("handle_mc_event: {event:?}"));
        match event {
            McEvent::Status(s) => self.install_status = s,
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
                self.persist_state();
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
                self.persist_state();
                self.edit.mod_input.clear();
                self.screen = Screen::Main;
            }
            KeyCode::Tab | KeyCode::Down => {
                self.config_focus = (self.config_focus + 1) % (CONFIG_MAX_FOCUS + 1);
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.config_focus = if self.config_focus == 0 {
                    CONFIG_MAX_FOCUS
                } else {
                    self.config_focus - 1
                };
            }
            KeyCode::Enter => match self.config_focus {
                0..=3 => self.config_focus = (self.config_focus + 1) % (CONFIG_MAX_FOCUS + 1),
                4 => {
                    if !self.edit.mod_input.is_empty() {
                        self.edit.mods.push(self.edit.mod_input.clone());
                        self.edit.mod_input.clear();
                    }
                }
                5 => {
                    let _ = open::that(lighty_launcher::core::AppState::data_dir());
                }
                6 => {
                    self.save_config();
                    self.persist_state();
                    self.screen = Screen::Main;
                }
                _ => {}
            },
            KeyCode::Char('d' | 'D') if key.modifiers == KeyModifiers::CONTROL => {
                if self.config_focus == 4 {
                    self.edit.mods.pop();
                }
            }
            KeyCode::Char(c) => self.type_config_char(c),
            KeyCode::Backspace => self.backspace_config(),
            _ => {}
        }
        false
    }

    fn type_config_char(&mut self, c: char) {
        let e = &mut self.edit;
        match self.config_focus {
            0 => { if !e.field_dirty[0] { e.username.clear(); e.field_dirty[0] = true; } e.username.push(c); }
            1 => { if !e.field_dirty[1] { e.min_ram.clear(); e.field_dirty[1] = true; } e.min_ram.push(c); }
            2 => { if !e.field_dirty[2] { e.max_ram.clear(); e.field_dirty[2] = true; } e.max_ram.push(c); }
            3 => { if !e.field_dirty[3] { e.jvm_args.clear(); e.field_dirty[3] = true; } e.jvm_args.push(c); }
            4 => { if !e.field_dirty[4] { e.mod_input.clear(); e.field_dirty[4] = true; } e.mod_input.push(c); }
            _ => {}
        }
    }

    fn backspace_config(&mut self) {
        let e = &mut self.edit;
        match self.config_focus {
            0 => { e.username.pop(); }
            1 => { e.min_ram.pop(); }
            2 => { e.max_ram.pop(); }
            3 => { e.jvm_args.pop(); }
            4 => { e.mod_input.pop(); }
            _ => {}
        }
    }

    fn handle_installing_key(&mut self, key: KeyEvent) -> bool {
        if let KeyCode::Char('c' | 'C') = key.code {
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
