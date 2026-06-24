/// Estado interno del launcher y lógica de entrada del teclado.
///
/// Este módulo es el "cerebro" de Nepix. Define:
/// - Los posibles estados de la aplicación (`Screen`)
/// - La estructura `App` que guarda todo el estado
/// - Cómo reacciona cada tecla según la pantalla actual
///
/// # Filosofía
///
/// Separamos el estado (`app.rs`) de la representación visual (`ui.rs`).
/// El `App` no sabe cómo se ve, solo qué datos tiene y qué hacer
/// cuando el usuario presiona una tecla. La TUI lee el estado y dibuja.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::Instant;
use tokio::sync::mpsc;

use crate::config::Config;
use crate::mc::{self, McEvent, VersionEntry};

/// Las diferentes pantallas o modos de la aplicación.
///
/// Solo puede haber una activa a la vez. La TUI revisa esto
/// para saber qué dibujar, y el manejador de teclado para
/// saber qué teclas responder.
#[derive(Debug, PartialEq)]
pub enum Screen {
    /// Pantalla principal: lista de versiones, selector, botón Launch.
    Main,
    /// Panel de configuración (se abre con Ctrl+P, se superpone a Main).
    Config,
    /// Pantalla de instalación con barra de progreso.
    Installing,
    /// Minecraft se está ejecutando. La TUI está restaurada (terminal normal).
    GameRunning,
}

/// Estado completo de la aplicación.
///
/// # Organización
///
/// - **Pantalla actual**: `screen` determina qué se ve y cómo se manejan teclas.
/// - **Config**: la configuración cargada del disco.
/// - **Versiones**: la lista obtenida de la API de Mojang.
/// - **Editores**: campos temporales para el panel de configuración
///   (no se guardan hasta que el usuario presiona "Save").
/// - **Instalación**: estado del progreso mientras se descarga/instala.
/// - **Canal MC**: tubería de comunicación con la tarea que lanza Minecraft.
pub struct App {
    // --- Pantalla ---
    pub screen: Screen,
    // --- Config ---
    pub config: Config,
    // --- Versiones ---
    pub versions: Vec<VersionEntry>,
    pub show_snapshots: bool,
    pub selected_version: usize,
    pub loader_idx: usize,

    // --- Campos de edición (Config screen) ---
    pub edit_username: String,
    pub edit_min_ram: String,
    pub edit_max_ram: String,
    pub edit_jvm_args: String,
    pub edit_mods: Vec<String>,
    pub edit_mod_input: String,
    /// Índice del campo enfocado en el panel de configuración.
    pub config_focus: usize,
    /// Cantidad total de campos editables en el panel.
    pub config_max_focus: usize,
    /// Marca qué campos han sido editados manualmente (para limpiar defaults al escribir).
    pub config_field_dirty: [bool; 6],

    // --- Estado de instalación ---
    pub install_status: String,
    pub install_progress: f64,
    pub install_total: u64,
    pub install_current: u64,
    pub install_start: Option<Instant>,
    pub install_has_real_progress: bool,

    // --- Canal de comunicación con la tarea de Minecraft ---
    pub mc_rx: mpsc::Receiver<McEvent>,
    pub mc_tx: mpsc::Sender<McEvent>,
    pub mc_task: Option<tokio::task::JoinHandle<()>>,

    // --- Error global (muestra al salir) ---
    pub error_message: Option<String>,
}

impl App {
    /// Crea una nueva aplicación con configuración y versiones cargadas.
    ///
    /// Selecciona automáticamente la última versión usada y el último loader.
    pub fn new(config: Config, versions: Vec<VersionEntry>) -> Self {
        let (mc_tx, mc_rx) = mpsc::channel(256);

        // Busca la última versión usada en la lista disponible
        let ver_idx = versions
            .iter()
            .position(|v| v.id == config.last_version)
            .unwrap_or(0);

        // Busca el último loader usado en la lista de loaders
        let loader_idx = mc::loader_list()
            .iter()
            .position(|l| **l == config.last_loader)
            .unwrap_or(0);

        let edit_mods = config.mods.clone();

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
        }
    }

    /// Devuelve la lista de versiones filtrada según `show_snapshots`.
    ///
    /// Si `show_snapshots` es `false`, solo muestra las de tipo "release".
    /// Limpia el mensaje de error mostrado en la TUI.
    /// Se llama cuando el usuario navega o cambia de pantalla.
    pub fn clear_error(&mut self) {
        self.error_message = None;
    }

    pub fn filtered_versions(&self) -> Vec<&VersionEntry> {
        self.versions
            .iter()
            .filter(|v| self.show_snapshots || v.version_type == "release")
            .collect()
    }

    /// Devuelve el ID de la versión actualmente seleccionada.
    ///
    /// Si no hay versiones, devuelve string vacío.
    pub fn current_version(&self) -> String {
        let list = self.filtered_versions();
        list.get(self.selected_version)
            .map(|v| v.id.clone())
            .unwrap_or_default()
    }

    /// Inicia el modo edición de configuración.
    ///
    /// Copia los valores actuales a los campos editables temporales.
    /// Si el usuario cancela, estos cambios se descartan.
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

    /// Guarda la configuración: copia los campos editables a la config real
    /// y persiste en disco.
    pub fn save_config(&mut self) {
        self.config.username = self.edit_username.clone();
        self.config.min_ram = self.edit_min_ram.clone();
        self.config.max_ram = self.edit_max_ram.clone();
        self.config.jvm_args = self.edit_jvm_args.clone();
        self.config.mods = self.edit_mods.clone();
        self.config.show_snapshots = self.show_snapshots;
        let _ = self.config.save();
    }

    /// Punto de entrada para el manejo de teclado.
    ///
    /// Redirige al método correspondiente según la pantalla actual.
    /// Devuelve `true` si la aplicación debe cerrarse.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        match self.screen {
            Screen::Main => self.handle_main_key(key),
            Screen::Config => self.handle_config_key(key),
            Screen::Installing => self.handle_installing_key(key),
            Screen::GameRunning => true,
        }
    }

    /// Maneja teclas en la pantalla principal.
    ///
    /// | Tecla       | Acción                               |
    /// |-------------|--------------------------------------|
    /// | Q           | Salir (guarda config)                |
    /// | ↑/k         | Versión anterior                     |
    /// | ↓/j         | Versión siguiente                    |
    /// | ←           | Loader anterior                      |
    /// | →           | Loader siguiente                     |
    /// | Ctrl+P      | Abrir panel de configuración         |
    /// | Ctrl+V      | Mostrar/ocultar snapshots            |
    /// | Enter       | Lanzar Minecraft                     |
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
            KeyCode::Enter => {
                let version = self.current_version();
                let username = self.config.username.clone();
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

                // Inicia una tarea asíncrona que ejecuta todo el proceso
                let tx = self.mc_tx.clone();
                let mods = self.config.mods.clone();
                self.mc_task = Some(tokio::spawn(async move {
                    mc::launch(username, version, loader, min_ram, max_ram, &jvm_args, &mods, tx).await;
                }));

                self.install_status = "Starting...".into();
                self.install_progress = 0.0;
                self.install_current = 0;
                self.install_total = 0;
                self.install_start = Some(Instant::now());
                self.install_has_real_progress = false;
                self.screen = Screen::Installing;
            }
            _ => {}
        }
        false
    }

    /// Maneja teclas en el panel de configuración.
    ///
    /// Los campos editables se comportan como minieditores de texto.
    /// El foco cambia con Tab, y Enter confirma la acción del campo actual.
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
                    if !self.config_field_dirty[0] {
                        self.edit_username.clear();
                        self.config_field_dirty[0] = true;
                    }
                    self.edit_username.push(c);
                }
                1 => {
                    if !self.config_field_dirty[1] {
                        self.edit_min_ram.clear();
                        self.config_field_dirty[1] = true;
                    }
                    self.edit_min_ram.push(c);
                }
                2 => {
                    if !self.config_field_dirty[2] {
                        self.edit_max_ram.clear();
                        self.config_field_dirty[2] = true;
                    }
                    self.edit_max_ram.push(c);
                }
                3 => {
                    if !self.config_field_dirty[3] {
                        self.edit_jvm_args.clear();
                        self.config_field_dirty[3] = true;
                    }
                    self.edit_jvm_args.push(c);
                }
                4 => {
                    if !self.config_field_dirty[4] {
                        self.edit_mod_input.clear();
                        self.config_field_dirty[4] = true;
                    }
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

    /// Maneja teclas durante la instalación.
    /// Solo responde a Ctrl+C para cancelar.
    /// Maneja teclas durante la instalación.
    ///
    /// Permite cancelar el proceso con `Ctrl+C`. Al hacerlo se aborta la
    /// tarea de lanzamiento y se limpia el estado de progreso/**/
    fn handle_installing_key(&mut self, key: KeyEvent) -> bool {
        if let KeyCode::Char('c') | KeyCode::Char('C') = key.code {
            if key.modifiers == KeyModifiers::CONTROL {
                // Intenta cancelar la tarea asíncrona de lanzamiento.
                if let Some(task) = self.mc_task.take() {
                    task.abort();
                }
                // Resetea la pantalla de instalación para volver al menú principal.
                self.install_status.clear();
                self.install_progress = 0.0;
                self.install_current = 0;
                self.install_total = 0;
                self.screen = Screen::Main;
            }
        }
        false
    }

    /// Procesa un evento recibido desde la tarea de Minecraft.
    ///
    /// Actualiza el estado según el tipo de evento:
    /// - `Status` / `Progress`: actualiza la pantalla de instalación.
    /// - `Launched`: cambia a `GameRunning`.
    /// - `ProcessExited`: vuelve a `Main`.
    /// - `Error`: guarda el error y vuelve a `Main`.
    pub fn handle_mc_event(&mut self, event: McEvent) {
        crate::log::info("APP", &format!("handle_mc_event: {event:?}"));
        match event {
            McEvent::Status(s) => {
                crate::log::info("PROGRESS", &format!("status={s}"));
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
                crate::log::info("PROGRESS", &format!("current={current} total={total} install_total={} install_progress={:.3}", self.install_total, self.install_progress));
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

    /// Avanza el progreso de mentiras cuando no hay info real.
    ///
    /// Sube lentamente de 0% a 85% durante ~30 segundos.
    /// Si después llegan eventos reales, el progreso real toma el control.
    pub fn advance_fake_progress(&mut self) {
        if self.install_has_real_progress {
            return;
        }
        if let Some(start) = self.install_start {
            let elapsed = start.elapsed().as_secs_f64();
            // Curva asintótica: 0 → 0.85 en ~30s
            self.install_progress = (1.0 - (-elapsed / 8.0).exp()) * 0.85;
        }
    }
}
