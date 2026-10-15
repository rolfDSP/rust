# Rust development image for r_proj_009.
# Used by .devcontainer/devcontainer.json, but also works with plain docker:
#   docker build -t r_proj_009-dev .
#   docker run --rm -it -v "$PWD":/workspace r_proj_009-dev
FROM mcr.microsoft.com/devcontainers/rust:1-bookworm

USER vscode
WORKDIR /workspace

# Download dependencies into the image so later builds work offline and start faster
COPY --chown=vscode:vscode Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs \
    && cargo fetch \
    && rm -rf src

# Project sources (replaced by the bind mount when used as a dev container)
COPY --chown=vscode:vscode src ./src

CMD ["bash"]
