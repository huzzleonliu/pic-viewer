FROM rust:bookworm AS builder
RUN rustup target add wasm32-unknown-unknown \
    && cargo install cargo-leptos --locked
WORKDIR /app
COPY Cargo.toml rust-toolchain.toml ./
COPY src ./src
COPY style ./style
COPY public ./public
RUN cargo leptos build --release

FROM debian:bookworm-slim
RUN useradd --uid 1000 --create-home --home-dir /app app \
    && apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /data \
    && chown app:app /data
COPY --from=builder --chown=1000:1000 /app/target/release/pic-viewer /app/pic-viewer
COPY --from=builder --chown=1000:1000 /app/target/site /app/site
USER app
WORKDIR /app
ENV LEPTOS_OUTPUT_NAME=pic-viewer \
    LEPTOS_SITE_ROOT=/app/site \
    LEPTOS_SITE_PKG_DIR=pkg \
    LEPTOS_SITE_ADDR=0.0.0.0:3000 \
    PIC_ROOT=/data
EXPOSE 3000
VOLUME ["/data"]
CMD ["/app/pic-viewer"]
