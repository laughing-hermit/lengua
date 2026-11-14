FROM node:24-bookworm AS check
ARG APP
ARG LIBRARIES=""
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none --profile minimal
ENV PATH=/root/.cargo/bin:$PATH
RUN npm install --global pnpm@12.18.0
WORKDIR /lengua
COPY rust-toolchain.toml .
RUN rustup toolchain install
RUN cargo install wasm-bindgen-cli --version 0.2.130 --locked
COPY . .
RUN cargo build --release --locked -p lengua-mcp -p lengua-run
ENV LENGUA_HOME=/lengua-home
RUN for part in $LIBRARIES $APP; do ./target/release/lengua-mcp check "$part" || exit 1; done
RUN cp target/release/lengua-run /lengua-run \
    && rm -rf target crates .github \
    && find . -type d \( -name node_modules -o -name deps -o -name incremental -o -name .fingerprint -o -name mutant -o -name mutant-harness -o -name harness \) -prune -exec rm -rf {} +

FROM debian:bookworm-slim
ARG APP
COPY --from=check /lengua-home /lengua-home
COPY --from=check /lengua-run /usr/local/bin/lengua-run
COPY --from=check /lengua /srv
ENV LENGUA_HOME=/lengua-home APP_FOLDER=/srv/${APP} PORT=80
CMD ["sh", "-c", "exec lengua-run --listen 0.0.0.0:${PORT} \"$APP_FOLDER\""]
