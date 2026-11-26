# egotop

**egotop** é um ambiente visual interativo de terminal (*terminal welcome dashboard*) escrito em Rust para Linux, Hyprland, Wayland e X11.

Execute apenas `egotop` — sem argumentos obrigatórios — para iniciar a janela principal com coleta real do sistema, animações de entrada, efeitos de fundo, personalização visual e linha de comandos inspirada no Neovim.

---

## Arquitetura

- **Window Engine** ([`src/window/mod.rs`](src/window/mod.rs)): gerenciamento independente da janela principal (e preparado para múltiplas janelas via `z_index`), com posição absoluta ou relativa, âncoras (`top-left`, `top-center`, `top-right`, `center`, `bottom-left`, `bottom-center`, `bottom-right`), movimentação por mouse/teclado (`draggable`), redimensionamento (`resizable`), bordas configuráveis (`rounded`, `solid`, `double`, `thick`), padding, visibilidade e fundo sólido ou transparente em nível de célula do terminal (`Color::Reset`).
- **Command Engine** ([`src/command/mod.rs`](src/command/mod.rs)): linha de comandos estilo Neovim oculta até pressionar `:`, com histórico (`↑`/`↓`) e autocompletar (`Tab`).
- **Animation Engine** ([`src/animation/transition.rs`](src/animation/transition.rs)): animações nativas de entrada/transição (`crt_power_on`, `curtains`, `fade`, `slide`, `matrix_boot`, `none`) além de efeitos contínuos de plano de fundo (`matrix`, `water`, `particles`, `scanlines`, `none`) e suporte a animações declarativas da comunidade em TOML (`schema_version = 1`).
- **Configuration & Theme Loaders** ([`src/config/`](src/config/)): tudo organizado sob `~/.config/egotop/` com carregamento em runtime (sem leitura de arquivos por frame).
- **System Collector** ([`src/system/`](src/system/)): coleta nativa via `/proc`, `/sys`, `procfs` e `libc`.

---

## Estrutura em `~/.config/egotop/`

```text
~/.config/egotop/
├── egotop.toml         # Configuração geral, tema ativo, animação inicial, layout e widgets
├── window.toml         # Posição, âncora, tamanho, bordas, fundo, padding e flags da janela
├── themes/             # Temas compartilháveis (*.toml)
├── animations/         # Animações declarativas da comunidade (*.toml, schema_version = 1)
├── fonts/              # Fontes FIGlet customizadas (*.flf)
└── layouts/            # Presets de layout
```

### Formato declarativo de animações da comunidade (`~/.config/egotop/animations/*.toml`)

```toml
schema_version = 1
name = "cyber_boot"
display_name = "Cyber Boot (Community)"
author = "egotop"
description = "Flash inicial CRT seguido de embaralhamento Matrix."

[[steps]]
effect = "crt_line"        # crt_line | curtains | fade | slide | matrix_scramble | flash | pause
duration_ms = 320
easing = "ease_out"        # linear | ease_in | ease_out | ease_in_out
target = "window"
color = "#39FF88"

[[steps]]
effect = "matrix_scramble"
duration_ms = 550
easing = "ease_in_out"
target = "content"
chars = "01ｱｶｻﾀﾅﾊ#$"
```

---

## Interação: Teclado, Mouse e Linha de Comandos

### Mouse & Interação Visual

- **Arrastar a borda superior** da janela move a janela (quando `draggable = true`).
- **Arrastar o canto inferior direito (`◢`)** redimensiona a janela (quando `resizable = true`).
- **Clicar em `[⚙]`** (ou pressionar `F2` / `g`) abre o painel visual interativo com abas para **Temas**, **Animações** (com pré-visualização ao vivo), **Janela** e **Layout**.
- **Clicar em `[?]`** (ou pressionar `h` / `?`) abre a ajuda.

### Comandos (`:` estilo Neovim)

Pressione `:` para abrir o prompt, `Tab` para autocompletar, `Enter` para executar e `Esc` para cancelar:

- `:colorscheme <nome>` (ou `:colorscheme` para abrir o seletor visual de temas)
- `:animation [nome]` (seleciona ou abre a interface interativa de pré-visualização/seleção de animações)
- `:window [center|anchor <pos>|border <on|off|rounded|solid|double|thick>|bg <solid|transparent>|drag <on|off>|resize <on|off>|padding <n>|reset]`
- `:layout [responsive|free|compact|minimal|dashboard]`
- `:effect [matrix|water|particles|scanlines|none]`
- `:reload` — recarrega configurações, temas, janela e animações de `~/.config/egotop/`
- `:help` — abre a ajuda
- `:q` — sai e restaura o terminal

### Atalhos Diretos

| Tecla | Ação |
| --- | --- |
| `:` | Abrir linha de comandos |
| `F2` / `g` | Abrir painel visual de personalização |
| `Setas` / `Shift+Setas` | Mover / redimensionar a janela |
| `r` / `p` / `c` | Atualizar dados / pausar animações / recarregar TOML |
| `m` / `e` / `1-3` | Modo de visualização / efeito de fundo / páginas |
| `q` / `Esc` / `Ctrl+C` | Fechar modal ou sair |

---

## Compilação, Testes e Integração

```bash
cd ~/projects/egotop
cargo test
cargo build --release
cargo install --path .
```

### Zsh (`~/.zshrc`) / Bash (`~/.bashrc`)

```zsh
if [[ -o interactive ]] && [[ -z "$EGOTOP_SHOWN" ]] && command -v egotop >/dev/null 2>&1; then
  export EGOTOP_SHOWN=1
  egotop
fi
```

### Hyprland (`~/.config/hypr/hyprland.conf`)

```ini
windowrulev2 = float, class:^(egotop-welcome)$
windowrulev2 = size 1150 720, class:^(egotop-welcome)$
windowrulev2 = center, class:^(egotop-welcome)$

exec-once = kitty --class egotop-welcome -e egotop
```
