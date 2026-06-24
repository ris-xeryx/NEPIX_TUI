/// Punto de entrada principal de Nepix.
///
/// # ¿Qué hace?
///
/// 1. Inicializa `AppState` (directorios de lighty-launcher)
/// 2. Carga la configuración guardada del usuario
/// 3. Obtiene la lista de versiones de Minecraft desde la API de Mojang
/// 4. Inicia la terminal en modo "raw" (TUI) con ratatui
/// 5. Ejecuta el bucle principal de eventos
/// 6. Cuando el usuario sale, restaura la terminal y guarda la config
///
/// # Bucle principal (`run`)
///
/// El bucle tiene dos fases:
///
/// ```text
/// Inicio → Cargar config → Fetch versiones → TUI loop
///   ↓                                              │
///   │  (Enter)                                      │ (Q)
///   ↓                                              │
/// Installing screen (progreso) ← ─ ─ ─ ─ ─ ─ ─ ─ ┘
///   ↓
///   (Minecraft launch)
///   ↓
/// GameRunning (terminal normal)
///   ↓
///   (Minecraft closes)
///   ↓
/// Vuelve a TUI loop ──────────────────────────────→ Salir
/// ```
///
/// Cuando el usuario pulsa Q, guarda la config y sale.
/// Cuando Minecraft se lanza (por Enter), se pasa a GameRunning.
/// Cuando el juego termina, se vuelve al launcher.
mod app;
mod config;
mod log;
mod mc;
mod ui;

use app::{App, Screen};
use crossterm::event::{self, Event};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = lighty_launcher::core::AppState::init("nepix");

    log::init(&lighty_launcher::core::AppState::data_dir().join("nepix.log"));

    let config = config::Config::load();
    let versions = mc::fetch_versions().await.unwrap_or_default();

    log::info("MAIN", &format!("Loaded {} versions", versions.len()));

    let mut terminal = ratatui::init();
    let mut app = App::new(config, versions);

    let res = run(&mut terminal, &mut app).await;

    ratatui::restore();
    if let Some(msg) = &app.error_message {
        eprintln!("Error: {msg}");
    }
    log::info("MAIN", "Exiting");
    res
}

/// Bucle principal de eventos.
///
/// Alterna entre dos modos:
/// - TUI activo: dibuja la interfaz y escucha teclas.
/// - GameRunning: libera la terminal y espera a que Minecraft se cierre.
fn enter_game_running(terminal: &mut ratatui::DefaultTerminal, app: &mut App) {
    let _ = terminal.draw(|frame| ui::render(frame, app));
    ratatui::restore();
    println!("\n  Nepix - Minecraft is running.");
    println!("  Close the game window to return to the launcher.\n");
}

async fn run(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
) -> anyhow::Result<()> {
    loop {
        // Procesa eventos de MC y auth
        while let Ok(event) = app.mc_rx.try_recv() {
            let was_installing = app.screen == Screen::Installing;
            app.handle_mc_event(event);

            if app.screen == Screen::Installing {
                app.advance_fake_progress();
                let _ = terminal.draw(|frame| ui::render(frame, app));
            }

            if was_installing && app.screen != Screen::Installing {
                log::info("MAIN", &format!("Installing→{:?}", app.screen));
                let _ = terminal.draw(|frame| ui::render(frame, app));
                match app.screen {
                    Screen::GameRunning => {
                        enter_game_running(terminal, app);
                        wait_for_game(app).await;
                        app.screen = Screen::Main;
                        *terminal = ratatui::init();
                        log::info("MAIN", "Back from wait_for_game, screen=Main");
                    }
                    _ => {}
                }
                break;
            }
        }

        while let Ok(auth_event) = app.auth_rx.try_recv() {
            app.handle_auth_event(auth_event);
            if app.screen == Screen::Installing {
                break;
            }
        }

        // Si el juego ya está corriendo, entra al wait
        if app.screen == Screen::GameRunning {
            enter_game_running(terminal, app);
            wait_for_game(app).await;
            app.screen = Screen::Main;
            *terminal = ratatui::init();
            log::info("MAIN", "Back from wait_for_game (via GameRunning guard), screen=Main");
            continue;
        }

        if app.screen == Screen::Installing {
            app.advance_fake_progress();
        }
        terminal.draw(|frame| ui::render(frame, app))?;

        // Espera hasta 16ms por una tecla. Sin sleep separado así la
        // respuesta al soltar una tecla es inmediata.
        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if app.handle_key(key) {
                    break;
                }
            }
        }
    }
    Ok(())
}

/// Espera a que el proceso de Minecraft termine.
///
/// Mientras espera, revisa eventos del canal `mc_rx`. Cuando recibe
/// `ProcessExited`, `Done` o `Error`, imprime el resultado y retorna.
/// También maneja el caso en que la tarea se cierra sin avisar.
async fn wait_for_game(app: &mut App) {
    loop {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(100)) => {}
            event = app.mc_rx.recv() => {
                match event {
                    Some(ref mc_event) => match mc_event {
                        crate::mc::McEvent::ProcessExited { exit_code } => {
                            println!("  Minecraft exited with code {exit_code}");
                            println!("  Returning to launcher...\n");
                        }
                        crate::mc::McEvent::Done => {
                            println!("  Minecraft closed.\n");
                        }
                        crate::mc::McEvent::Error(e) => {
                            println!("  Error: {e}\n");
                        }
                        _ => {}
                    },
                    None => {
                        println!("  Launcher task ended unexpectedly.\n");
                    }
                }
                app.handle_mc_event(event.unwrap_or(crate::mc::McEvent::Done));
                return;
            }
        }
    }
}
