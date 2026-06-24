/// Configuración persistente del launcher Nepix.
///
/// Guarda y carga la configuración del usuario en un archivo TOML
/// ubicado en `~/.config/nepix/config.toml`. Usa la librería `serde`
/// para convertir los datos entre Rust y TOML.
///
/// # Ejemplo de archivo generado
///
/// ```toml
/// username = "Player123"
/// min_ram = "2G"
/// max_ram = "4G"
/// jvm_args = "-XX:+UseG1GC"
/// last_version = "1.21.4"
/// last_loader = "Vanilla"
/// show_snapshots = false
/// mods = ["fabric-api", "sodium"]
/// ```

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Representa toda la configuración del launcher.
///
/// Cada campo se guarda y carga desde el archivo `config.toml`.
/// Los campos con `Option` pueden estar vacíos si el usuario nunca los configuró.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Nombre de usuario para modo offline (sin cuenta Microsoft).
    pub username: String,
    /// Memoria RAM mínima para la JVM (ej: "512M", "2G").
    pub min_ram: String,
    /// Memoria RAM máxima para la JVM (ej: "2G", "4G", "8G").
    pub max_ram: String,
    /// Argumentos adicionales para la JVM, separados por coma.
    /// Ejemplo: "-XX:+UseG1GC,-XX:+ParallelRefProcEnabled"
    pub jvm_args: String,
    /// Última versión de Minecraft seleccionada (para recordar al abrir).
    pub last_version: String,
    /// Último loader seleccionado ("Vanilla", "Fabric", "Quilt", etc.).
    pub last_loader: String,
    /// Si es `true`, se muestran versiones snapshot (no solo releases).
    pub show_snapshots: bool,
    /// Lista de mods (slugs de Modrinth) que el usuario ha agregado.
    pub mods: Vec<String>,
}

impl Default for Config {
    /// Valores por defecto para un usuario que abre el launcher por primera vez.
    fn default() -> Self {
        Self {
            username: "Player".into(),
            min_ram: "2G".into(),
            max_ram: "4G".into(),
            jvm_args: "-XX:+UseG1GC".into(),
            last_version: String::new(),
            last_loader: "Vanilla".into(),
            show_snapshots: false,
            mods: Vec::new(),
        }
    }
}

impl Config {
    /// Devuelve la ruta al directorio de configuración de Nepix.
    ///
    /// En Linux: `~/.config/nepix/`
    /// En macOS: `~/Library/Application Support/nepix/`
    /// En Windows: `C:\Users\usuario\AppData\Roaming\nepix\`
    ///
    /// Usa la librería `directories` para encontrar la ruta correcta
    /// según el sistema operativo.
    pub fn config_dir() -> PathBuf {
        directories::ProjectDirs::from("", "", "nepix")
            .map(|d| d.config_dir().to_path_buf())
            .unwrap_or_else(|| {
                // Si no se puede determinar, usamos ~/.config/nepix como fallback
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
                PathBuf::from(home).join(".config").join("nepix")
            })
    }

    /// Carga la configuración desde el archivo TOML.
    ///
    /// Si el archivo no existe o está corrupto, devuelve los valores
    /// por defecto. El launcher siempre puede arrancar aunque no haya
    /// configuración previa.
    pub fn load() -> Self {
        let path = Self::config_dir().join("config.toml");
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|c| toml::from_str(&c).ok())
            .unwrap_or_default()
    }

    /// Guarda la configuración actual al archivo TOML.
    ///
    /// Crea el directorio si no existe. Falla silenciosamente
    /// si no se puede escribir (el launcher sigue funcionando
    /// con la config en memoria).
    pub fn save(&self) -> anyhow::Result<()> {
        let dir = Self::config_dir();
        std::fs::create_dir_all(&dir)?;
        let content = toml::to_string_pretty(self)?;
        std::fs::write(dir.join("config.toml"), &content)?;
        Ok(())
    }
}
