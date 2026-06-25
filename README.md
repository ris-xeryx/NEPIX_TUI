# Nepix

> Launcher de Minecraft para terminal. Rápido, ligero, sin GUI pesada.

---

## 🚀 Instalación

### Linux / macOS

```bash
curl -fsSL https://github.com/ris-xeryx/NEPIX_TUI/releases/latest/download/install.sh | bash
```

### Windows (PowerShell)

```powershell
irm https://github.com/ris-xeryx/NEPIX_TUI/releases/latest/download/install.ps1 | iex
```

---

## Uso

```bash
nepix
```

- `↑↓` o `jk` — Navegar versiones
- `Enter` — Lanzar con Microsoft login (modo online)
- `o` — Lanzar offline (sin auth)
- `Ctrl+M` — Alternar online/offline
- `q` — Salir

> **Nota:** El modo online requiere aprobación de Mojang. Puede tardar semanas en activarse.
> Mientras tanto, usa el modo offline con `o`.

---

## Compilar desde fuente

```bash
git clone https://github.com/ris-xeryx/NEPIX_TUI
cd NEPIX_TUI
cargo build --release
```

---

## Probado en

| Componente | Detalle |
|-----------|---------|
| **SO** | Fedora Workstation 44 |
| **Kernel** | 6.19.10-300.fc44.x86_64 |
| **CPU** | Intel Core i3-4170 (4 núcleos) @ 3.70GHz |
| **RAM** | 7.6 GiB |
| **GPU** | Intel HD Graphics 4400 (integrada) |

---

## 🤝 Contribuir

Este proyecto está abierto a sugerencias, ideas, reportes de bugs y pull requests.

- **Issues:** [github.com/ris-xeryx/NEPIX_TUI/issues](https://github.com/ris-xeryx/NEPIX_TUI/issues)
- **Discusiones:** Cualquier idea es bienvenida en la pestaña de Issues.
- **PRs:** Haz fork, crea rama, envía PR. Sin formalismos.

---

## Licencia

[GNU General Public License v3.0](LICENSE)
