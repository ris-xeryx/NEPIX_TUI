/// Operaciones con Minecraft: obtener versiones, lanzar el juego.
///
/// Este módulo es el puente entre Nepix y `lighty-launcher`, la librería
/// que sabe cómo descargar e iniciar Minecraft. También se encarga de
/// obtener la lista de versiones disponibles desde la API de Mojang.
///
/// # ¿Cómo funciona?
///
/// 1. `fetch_versions()` llama a la API de Mojang y devuelve todas las
///    versiones de Minecraft (releases y snapshots).
/// 2. `launch()` arma todo lo necesario: autenticación offline, el
///    `VersionBuilder` con la versión y loader elegidos, y lanza el juego.
/// 3. Durante el lanzamiento, envía eventos por un canal (`mpsc`) para
///    que la TUI pueda mostrar el progreso en tiempo real.

use anyhow::Result;
use lighty_launcher::prelude::*;
use serde::Deserialize;
use tokio::sync::mpsc;

/// Una versión de Minecraft disponible para descargar.
///
/// Contiene solo la información necesaria para mostrar la lista
/// y lanzar la versión seleccionada.
#[derive(Debug, Clone)]
pub struct VersionEntry {
    /// Identificador de la versión (ej: "1.21.4", "25w14a").
    pub id: String,
    /// Tipo de versión: "release" para versiones estables,
    /// "snapshot" para versiones de prueba.
    pub version_type: String,
}

/// Eventos que el proceso de lanzamiento envía a la TUI.
///
/// Funciona como un "radio" entre el launcher y la interfaz:
/// el módulo `mc.rs` transmite, la TUI recibe y actualiza la pantalla.
///
/// # Variantes
/// - `Status`: mensaje de texto (ej: "Descargando...")
/// - `Progress`: bytes descargados vs total
/// - `Launched`: el juego arrancó correctamente
/// - `ProcessOutput`: una línea de la consola del juego
/// - `ProcessExited`: el juego se cerró
/// - `Error`: algo salió mal
/// - `Done`: proceso terminado sin errores
#[derive(Debug)]
#[allow(dead_code)]
pub enum McEvent {
    /// Mensaje de estado actual (ej: "Descargando assets...").
    Status(String),
    /// Progreso de descarga en bytes.
    Progress {
        /// Bytes descargados en este paso.
        current: u64,
        /// Total de bytes a descargar (0 si se desconoce).
        total: u64,
    },
    /// El juego se lanzó exitosamente. Incluye el PID del proceso.
    Launched {
        /// Process ID de Minecraft.
        pid: u32,
    },
    /// Una línea de salida de la consola de Minecraft (stdout/stderr).
    ProcessOutput(String),
    /// El proceso de Minecraft terminó.
    ProcessExited {
        /// Código de salida (0 = normal, otro = error).
        exit_code: i32,
    },
    /// Error durante el proceso de lanzamiento.
    Error(String),
    /// Proceso completado (Minecraft cerró sin errores reportados).
    Done,
}

/// Estructura interna para deserializar el JSON de la API de Mojang.
#[derive(Deserialize)]
struct Manifest {
    versions: Vec<ManifestVersion>,
}

/// Una entrada individual en el manifiesto de versiones de Mojang.
#[derive(Deserialize)]
struct ManifestVersion {
    id: String,
    #[serde(rename = "type")]
    version_type: String,
}

/// Obtiene la lista completa de versiones de Minecraft desde la API de Mojang.
///
/// Hace una petición HTTP GET a:
/// `https://launchermeta.mojang.com/mc/game/version_manifest_v2.json`
///
/// La respuesta es un JSON con todas las versiones que han existido
/// (desde la 1.0 hasta las más recientes).
///
/// # Errores
/// - Si no hay internet, devuelve un error.
/// - Si la API cambia su formato, puede fallar la deserialización.
///
/// El llamador (main.rs) maneja el error con `unwrap_or_default()`,
/// devolviendo una lista vacía si algo falla.
pub async fn fetch_versions() -> Result<Vec<VersionEntry>> {
    let resp = reqwest::get("https://launchermeta.mojang.com/mc/game/version_manifest_v2.json")
        .await?
        .json::<Manifest>()
        .await?;

    Ok(resp
        .versions
        .into_iter()
        .map(|v| VersionEntry {
            id: v.id,
            version_type: v.version_type,
        })
        .collect())
}

/// Convierte un nombre de loader (string) al enum `Loader` de lighty-launcher.
///
/// # Ejemplos
/// - `"Fabric"` → `Loader::Fabric`
/// - `"Vanilla"` → `Loader::Vanilla`
/// - `"desconocido"` → `Loader::Vanilla` (por defecto)
fn parse_loader(s: &str) -> Loader {
    match s {
        "Fabric" => Loader::Fabric,
        "Quilt" => Loader::Quilt,
        "NeoForge" => Loader::NeoForge,
        "Forge" => Loader::Forge,
        _ => Loader::Vanilla,
    }
}

/// Lista de nombres de loaders soportados, para mostrar en la TUI.
///
/// El orden aquí determina el orden en que aparecen en el selector.
pub fn loader_list() -> &'static [&'static str] {
    &["Vanilla", "Fabric", "Quilt", "NeoForge", "Forge"]
}

/// Resuelve la version mas reciente del loader para una version de Minecraft dada.
///
/// Para Vanilla devuelve `""` (no necesita version de loader).
/// Para los demas loaders consulta sus APIs publicas.
async fn resolve_loader_version(loader_name: &str, mc_version: &str) -> Option<String> {
    match loader_name {
        "Fabric" => resolve_fabric(mc_version).await,
        "Quilt" => resolve_quilt(mc_version).await,
        "Forge" => resolve_forge(mc_version).await,
        "NeoForge" => resolve_neoforge(mc_version).await,
        _ => Some(String::new()),
    }
}

#[derive(Deserialize)]
struct FabricLoaderEntry {
    loader: FabricLoaderVersion,
}

#[derive(Deserialize)]
struct FabricLoaderVersion {
    version: String,
}

async fn resolve_fabric(mc_version: &str) -> Option<String> {
    let url = format!("https://meta.fabricmc.net/v2/versions/loader/{mc_version}");
    let resp: Vec<FabricLoaderEntry> = reqwest::get(&url).await.ok()?.json().await.ok()?;
    resp.into_iter().next().map(|e| e.loader.version)
}

#[derive(Deserialize)]
struct QuiltLoaderEntry {
    loader: QuiltLoaderVersion,
}

#[derive(Deserialize)]
struct QuiltLoaderVersion {
    version: String,
}

async fn resolve_quilt(mc_version: &str) -> Option<String> {
    let url = format!("https://meta.quiltmc.org/v3/versions/loader/{mc_version}");
    let resp: Vec<QuiltLoaderEntry> = reqwest::get(&url).await.ok()?.json().await.ok()?;
    resp.into_iter().next().map(|e| e.loader.version)
}

#[derive(Deserialize)]
struct ForgePromotions {
    promos: std::collections::HashMap<String, String>,
}

async fn resolve_forge(mc_version: &str) -> Option<String> {
    let url = "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
    let resp: ForgePromotions = reqwest::get(url).await.ok()?.json().await.ok()?;
    resp.promos
        .get(&format!("{mc_version}-recommended"))
        .or_else(|| resp.promos.get(&format!("{mc_version}-latest")))
        .cloned()
}

#[derive(Deserialize)]
struct NeoForgeVersions {
    versions: Vec<String>,
}

async fn resolve_neoforge(mc_version: &str) -> Option<String> {
    let url = "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";
    let resp: NeoForgeVersions = reqwest::get(url).await.ok()?.json().await.ok()?;
    let parts: Vec<u32> = mc_version
        .split('.')
        .filter_map(|p| p.parse().ok())
        .collect();
    let major = parts.first().copied().unwrap_or(0);
    let minor = parts.get(1).copied().unwrap_or(0);
    resp.versions
        .into_iter()
        .rev()
        .find(|v| {
            let v_parts: Vec<u32> = v.split('.').filter_map(|p| p.parse().ok()).collect();
            v_parts.first().copied() == Some(minor)
                || (major == 1 && minor > 0 && v_parts.first().copied() == Some(minor))
        })
}

/// Función principal de lanzamiento.
///
/// # ¿Qué hace?
///
/// 1. **Autenticación**: crea una sesión offline con el nombre de usuario.
/// 2. **Construcción**: arma un `VersionBuilder` con versión, loader y nombre.
/// 3. **Eventos**: crea un `EventBus` para recibir eventos de progreso.
/// 4. **Lanzamiento**: llama a `launch_builder.run()` que:
///    - Descarga archivos si es necesario (librerías, assets, natives)
///    - Busca/descarga Java (Temurin) automáticamente
///    - Inicia el proceso de Minecraft
///    - Espera a que el juego se cierre
///
/// # El canal de eventos
///
/// Mientras todo esto ocurre, un `tokio::spawn` escucha el `EventBus`
/// y reenvía los eventos al canal `mpsc` que la TUI está vigilando.
/// Así la interfaz se actualiza sola sin necesidad de "preguntar".
///
/// # Parámetros
/// - `username`: nombre para la cuenta offline
/// - `version`: versión de Minecraft (ej: "1.21.4")
/// - `loader_name`: "Vanilla", "Fabric", etc.
/// - `min_ram`: RAM mínima (ej: "2G")
/// - `max_ram`: RAM máxima (ej: "4G")
/// - `jvm_args`: argumentos JVM adicionales
/// - `mods`: slugs de Modrinth a instalar
/// - `tx`: canal para enviar eventos de vuelta a la TUI
pub async fn launch(
    username: String,
    version: String,
    loader_name: String,
    min_ram: String,
    max_ram: String,
    jvm_args: &[String],
    mods: &[String],
    tx: mpsc::Sender<McEvent>,
) {
    let _ = tx.send(McEvent::Status("Initializing...".into())).await;

    // Autenticación offline: no necesita Microsoft, solo un nombre
    let mut auth = OfflineAuth::new(&username);
    let profile = match auth.authenticate(None::<&EventBus>).await {
        Ok(p) => p,
        Err(e) => {
            let _ = tx.send(McEvent::Error(format!("Auth failed: {e}"))).await;
            return;
        }
    };

    let loader = parse_loader(&loader_name);

    let loader_version = match resolve_loader_version(&loader_name, &version).await {
        Some(v) => v,
        None => {
            let msg = format!(
                "No {} version found for Minecraft {}",
                loader_name, version
            );
            let _ = tx.send(McEvent::Error(msg)).await;
            return;
        }
    };

    crate::log::info("LOADER", &format!("resolved {loader_name} version: {loader_version}"));

    // VersionBuilder: el "plano" de la instancia de Minecraft
    // Parámetros: (nombre, loader, version_del_loader, version_de_minecraft)
    let mut instance = VersionBuilder::new(
        &format!("nepix-{version}-{}", loader_name.to_lowercase()),
        loader,
        &loader_version,
        &version,
    );

    // EventBus: sistema de mensajería entre lighty-launcher y nosotros
    let bus = EventBus::new(1000);
    let mut rx = bus.subscribe();

    // Hilo escucha-eventos: reenvía eventos del bus al canal de la TUI
    let tx_clone = tx.clone();
    tokio::spawn(async move {
        let mut downloaded: u64 = 0;
        loop {
            match rx.next().await {
                Ok(Event::Launch(LaunchEvent::InstallStarted { total_bytes, .. })) => {
                    downloaded = 0;
                    crate::log::info("MC-EVENT", &format!("InstallStarted total_bytes={total_bytes}"));
                    let _ = tx_clone
                        .send(McEvent::Progress {
                            current: 0,
                            total: total_bytes,
                        })
                        .await;
                    let _ = tx_clone.send(McEvent::Status("Downloading...".into())).await;
                }
                Ok(Event::Launch(LaunchEvent::InstallProgress { bytes })) => {
                    downloaded += bytes;
                    let _ = tx_clone
                        .send(McEvent::Progress {
                            current: downloaded,
                            total: 0,
                        })
                        .await;
                }
                Ok(Event::Launch(LaunchEvent::InstallCompleted { total_bytes, .. })) => {
                    crate::log::info("MC-EVENT", "InstallCompleted");
                    let _ = tx_clone
                        .send(McEvent::Progress {
                            current: total_bytes,
                            total: total_bytes,
                        })
                        .await;
                    let _ = tx_clone
                        .send(McEvent::Status("Install complete. Launching...".into()))
                        .await;
                }
                Ok(Event::Launch(LaunchEvent::Launched { pid, .. })) => {
                    crate::log::info("MC-EVENT", &format!("Launched pid={pid}"));
                    let _ = tx_clone.send(McEvent::Launched { pid }).await;
                }
                Ok(Event::Launch(LaunchEvent::ProcessOutput { line, .. })) => {
                    let _ = tx_clone.send(McEvent::ProcessOutput(line)).await;
                }
                Ok(Event::Launch(LaunchEvent::ProcessExited { exit_code, .. })) => {
                    crate::log::info("MC-EVENT", &format!("ProcessExited code={exit_code}"));
                    let _ = tx_clone.send(McEvent::ProcessExited { exit_code }).await;
                    break;
                }
                Ok(Event::Launch(LaunchEvent::IsInstalled { .. })) => {
                    let _ = tx_clone
                        .send(McEvent::Status("Already installed, preparing Java...".into()))
                        .await;
                }
                Ok(Event::Launch(LaunchEvent::Launching { .. })) => {
                    let _ = tx_clone
                        .send(McEvent::Status("Launching Minecraft...".into()))
                        .await;
                }
                Ok(Event::Launch(LaunchEvent::NotLaunched { error, .. })) => {
                    crate::log::info("MC-EVENT", &format!("NotLaunched error={error}"));
                    let _ = tx_clone.send(McEvent::Error(error)).await;
                    break;
                }
                Ok(other) => {
                    crate::log::info("MC-EVENT", &format!("Unexpected event: {other:?}"));
                }
                Err(_) => break,
            }
        }
    });

    // Aplicar mods si hay y el loader no es Vanilla
    let is_not_vanilla = loader_name != "Vanilla";
    if !mods.is_empty() && is_not_vanilla {
        let _ = tx
            .send(McEvent::Status(format!("Adding {} mod(s)...", mods.len())))
            .await;
        instance = instance
            .with_mod()
            .with_modrinth_mods(mods.iter().map(|m| (m.clone(), None::<String>)).collect())
            .done();
    }

    // RAM y argumentos JVM adicionales
    let natives_path = instance.game_dirs().join("natives").display().to_string();
    let mut launch_builder = instance.launch(&profile, JavaDistribution::Temurin);
    let mut jvm = launch_builder.with_event_bus(&bus).with_jvm_options();
    jvm = jvm.set("Xmx", &max_ram).set("Xms", &min_ram);
    jvm = jvm.set("Djava.library.path", &natives_path);
    for arg in jvm_args {
        let trimmed = arg.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=') {
            let clean_key = key.trim_start_matches('-');
            jvm = jvm.set(clean_key, value);
        } else {
            let clean = trimmed.trim_start_matches('-');
            jvm = jvm.set(clean, "");
        }
    }
    launch_builder = jvm.done();

    // Asegura que el event listener (spawned arriba) se suscriba
    // antes de que run() dispare eventos. Sin esto, versiones cacheadas
    // donde run() retorna casi instantáneamente pierden todos los eventos.
    tokio::task::yield_now().await;

    // run() bloquea esta tarea hasta que Minecraft se cierre
    match launch_builder.run().await {
        Ok(()) => {
            let _ = tx.send(McEvent::Done).await;
        }
        Err(e) => {
            let _ = tx.send(McEvent::Error(format!("Launch failed: {e}"))).await;
        }
    }
}
