# Nepix

Launcher de Minecraft para terminal. Rápido, ligero, sin GUI pesada.

## Instalación

```bash
curl -fsSL https://github.com/ris-xeryx/NEPIX_TUI/releases/latest/download/install.sh | bash
```

Windows (PowerShell):

```powershell
irm https://github.com/ris-xeryx/NEPIX_TUI/releases/latest/download/install.ps1 | iex
```

## Uso

```bash
nepix
```

- Flechas para navegar versiones
- Enter para instalar y lanzar
- Q para salir

## Requisitos

- Java 17+ (para correr Minecraft)
- Conexión a internet (descarga de versiones)

## Compilar desde fuente

```bash
git clone https://github.com/ris-xeryx/NEPIX_TUI
cd nepix
cargo build --release
```

## Licencia

MIT
