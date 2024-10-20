FROM alpine:3.20 AS builder
ENV RUSTFLAGS="-C target-feature=-crt-static -C target-cpu=native"
RUN apk add --no-cache musl-dev curl gcc
WORKDIR /pumpkin
COPY . /pumpkin

# Install Rust
RUN curl --proto '=https' --tlsv1.3 -sSf https://sh.rustup.rs | sh -s -- -y &&\
    source $HOME/.cargo/env &&\
    rustc -V &&\
    cargo -V &&\
# Build Pumpkin
    mount=type=cache,sharing=private,target=/pumpkin/target &&\
    mount=type=cache,target=/usr/local/cargo/git/db &&\
    mount=type=cache,target=/usr/local/cargo/registry/ &&\
    cargo build --release && cp target/release/pumpkin ./pumpkin.release
RUN strip pumpkin.release && ls -lsah pumpkin.release

FROM alpine:3.20
LABEL org.opencontainers.image.source=https://github.com/knight1/pumpkin
WORKDIR /pumpkin
RUN apk add --no-cache libgcc
COPY --from=builder /pumpkin/pumpkin.release /pumpkin/pumpkin
ENV RUST_BACKTRACE=1
EXPOSE 25565
ENTRYPOINT ["/pumpkin/pumpkin"]
