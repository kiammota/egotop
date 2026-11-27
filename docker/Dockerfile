FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    TERM=xterm-256color \
    COLORTERM=truecolor

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    ca-certificates \
    cmake \
    curl \
    git \
    htop \
    iproute2 \
    liblua5.4-dev \
    libssl-dev \
    lua5.4 \
    nano \
    pkg-config \
    procps \
    tmux \
    unzip \
    vim \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile default \
    && rustup component add clippy rustfmt \
    && cargo --version \
    && rustc --version

WORKDIR /workspace

CMD ["/bin/bash"]
