# egotop

**egotop** é um dashboard interativo de terminal (*terminal welcome dashboard*) escrito em Rust para ambientes Linux — com atenção especial para setups **Hyprland**, **Wayland**, **Kitty**, **Alacritty**, **WezTerm** e **foot**.

Ele combina a coleta nativa de informações do sistema (semelhante ao Fastfetch) com um motor de animações em plano de fundo, fontes ASCII no estilo FIGlet, layout totalmente personalizável em TOML e controles interativos.

---

## Recursos

- **Coleta nativa no Linux** (sem `sudo`, sem executar comandos externos repetidamente):
  - **Sistema**: distribuição (`/etc/os-release`), kernel, arquitetura, hostname, usuário, shell e versão, terminal, DE/WM, sessão Wayland/X11 e dimensões do terminal.
  - **Hardware**: modelo da CPU, utilização real em tempo real (delta `/proc/stat`), temperatura via `hwmon`/`thermal_zone`, RAM e Swap (`/proc/meminfo`), armazenamento (`/proc/mounts` + `statvfs`) e GPU/VRAM via `/sys/class/drm`.
  - **Atividade**: load average (`1m / 5m / 15m`), processos, threads e sparklines em tempo real de CPU% e RAM%.
  - **Sessão & Uptime**: tempo ligado, horário de inicialização e hora/data configuráveis.
- **Motor extensível de animações de fundo**:
  - `matrix`: chuva de caracteres caindo verticalmente (Katakana, Hex, ASCII ou Binary), densidade, comprimento e região configuráveis.
  - `water`: ondas animadas em interferência contínua com velocidade, amplitude e densidade.
  - `particles`: partículas flutuantes independentes.
  - `scanlines`: linhas horizontais CRT com varredura contínua.
  - `none`: fundo estático.
- **Tipografia ASCII & Logos**:
  - Fontes integradas: `ANSI Shadow`, `Standard`, `Big`, `Slant`, `Small`, `Block`, `Doom`.
  - Suporte a arquivos `.flf` customizados em `~/.config/egotop/fonts/`.
  - Fallback automático para ASCII quando `ascii_fallback = true` ou `use_nerd_fonts = false`.
  - Logos integrados (`auto`, `arch`, `hyprland`, `linux`, `egotop`) ou logo customizado inline/arquivo.
  - Efeito gradual de revelação estilo Matrix na saudação inicial.
- **Layout flexível e responsivo**:
  - Posicionamento por âncoras (`top-left`, `top-center`, `top-right`, `center-left`, `center`, `center-right`, `bottom-left`, `bottom-center`, `bottom-right`, `absolute`).
  - Largura e altura em células ou porcentagem (`"50%"`).
  - Ordem de renderização configurável por `z_index`.
  - Modos `responsive`, `free` e `compact` que se adaptam ao redimensionamento da janela sem quebrar.

---

## Instalação e Execução

```bash
cd ~/projects/egotop
cargo build --release
./target/release/egotop
```

Para instalar no PATH local do Cargo (`~/.cargo/bin/egotop`):

```bash
cargo install --path .
```

### Opções de Linha de Comando

```bash
egotop                          # Inicia com ~/.config/egotop.toml (cria padrão se não existir)
egotop --config /caminho.toml   # Usa um arquivo de configuração específico
egotop --effect water           # Sobrescreve o efeito de fundo na sessão atual
egotop --page 2                 # Abre diretamente na página 2
egotop --check-config           # Valida o TOML e sai
egotop --print-default-config   # Imprime a configuração padrão no stdout
```

---

## Controles Interativos

| Tecla | Ação |
| --- | --- |
| `q` / `Esc` / `Ctrl+C` | Sair e restaurar o terminal |
| `r` | Atualizar informações do sistema imediatamente |
| `p` | Pausar / retomar animações |
| `c` | Recarregar `~/.config/egotop.toml` sem reiniciar |
| `m` | Alternar modo de visualização (`Dashboard` / `Compacto` / `Minimal`) |
| `e` | Alternar efeito de fundo (`Matrix` / `Water` / `Particles` / `Scanlines` / `None`) |
| `1`, `2`, `3` | Alternar entre páginas do dashboard |
| `h` / `?` | Abrir ou fechar o painel de ajuda |

---

## Configuração (`~/.config/egotop.toml`)

Na primeira execução, o `egotop` cria automaticamente `~/.config/egotop.toml` com o arquivo padrão documentado em [`config/default.toml`](config/default.toml).

### Seções principais

- `[general]`: `name`, `greeting` (placeholders `{user}`, `{host}`, `{distro}`, `{wm}`), `user_override`, `message`, `refresh_rate_ms`, `animation_fps`, `time_format`, `date_format`, `hide_unavailable`, `view_mode`, `default_page`.
- `[theme]`: presets (`matrix`, `cyberpunk`, `nord`, `amber`, `dracula`, `custom`), `background` (aceita `#RRGGBB`, nome de cor ou `"reset"` para transparência do terminal), `panel_background`, `primary`, `secondary`, `accent`, `text`, `muted`, `border`, `danger`, `border_type` (`rounded`, `plain`, `double`, `thick`).
- `[typography]`: `font`, `logo_font`, `use_nerd_fonts`, `ascii_fallback`, `logo_preset`, `custom_logo`, `custom_logo_file`.
- `[header]`: `enabled`, `show_clock`, `show_date`, `show_indicators`, `show_fps`.
- `[background]` e `[background.<efeito>]`: `effect`, `enabled`, `speed`, `density`, `opacity` e `region` opcional (`{ x, y, width, height, unit = "percent" | "cells" }`).
- `[layout]`: `mode` (`responsive`, `free`, `compact`), `padding`, `gap`, `min_width_for_split`.
- `[[widgets]]`: lista de painéis independentes (`greeting`, `logo`, `system`, `hardware`, `activity`, `session`, `uptime`, `message`) com `enabled`, `title`, `position`, `page`, `width`, `height`, `x`, `y`, `offset_x`, `offset_y`, `align`, `z_index`, `font`, `color`, `border_color`, `show_border`, `fields`, `graph_style`, `reveal_animation`.

---

## Integração com Zsh, Bash e Hyprland

> O `egotop` **não modifica** seus arquivos de configuração automaticamente. Use os exemplos abaixo conforme sua preferência.

### Zsh (`~/.zshrc`)

Para exibir o `egotop` ao abrir um novo terminal interativo (sem atrapalhar subshells ou scripts):

```zsh
if [[ -o interactive ]] && [[ -z "$EGOTOP_SHOWN" ]] && command -v egotop >/dev/null 2>&1; then
  export EGOTOP_SHOWN=1
  egotop
fi
```

### Bash (`~/.bashrc`)

```bash
if [[ $- == *i* ]] && [[ -z "$EGOTOP_SHOWN" ]] && command -v egotop >/dev/null 2>&1; then
  export EGOTOP_SHOWN=1
  egotop
fi
```

### Hyprland (`~/.config/hypr/hyprland.conf`)

Para abrir automaticamente uma janela de boas-vindas com o Kitty ao iniciar a sessão do Hyprland:

```ini
# Abre um terminal flutuante de boas-vindas com o egotop no login
windowrulev2 = float, class:^(egotop-welcome)$
windowrulev2 = size 1150 720, class:^(egotop-welcome)$
windowrulev2 = center, class:^(egotop-welcome)$

exec-once = kitty --class egotop-welcome -e egotop
```

Ou crie um atalho dedicado no Hyprland (ex.: `SUPER + W`):

```ini
bind = $mainMod, W, exec, kitty --class egotop-welcome -e egotop
```

---

## Testes

```bash
cargo test
```
