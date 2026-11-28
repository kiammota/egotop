# Ambiente Linux com Rust & Cargo (Docker)

Este ambiente Docker fornece um container Ubuntu 24.04 completo com `rustup`, `cargo`, `rustc`, `clippy`, `rustfmt`, `build-essential`, `pkg-config`, `cmake`, `lua5.4`, `procps`, `git` e `tmux` para compilar, testar e usar projetos Rust no Linux. O egotop suporta Linux e macOS; Windows não é suportado.

## Uso rápido com Docker Compose (recomendado)

Na pasta `docker/` em um host Linux ou macOS com Docker Compose:

```bash
docker compose build
docker compose run --rm linux-rust
```

Dentro do container (`/workspace` montado no repositório):

```bash
cargo test
cargo clippy --all-targets
cargo run
```

## Uso independente do projeto (sandbox genérico Linux + Cargo)

Para abrir um shell Linux limpo com Cargo em qualquer pasta:

```bash
docker build -t linux-cargo-dev -f docker/Dockerfile docker/
docker run --rm -it -e TERM=xterm-256color -e COLORTERM=truecolor linux-cargo-dev
```

Ou montando o diretório atual:

```bash
docker run --rm -it -v "$(pwd):/workspace" -w /workspace linux-cargo-dev
```
