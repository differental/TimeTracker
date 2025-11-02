# syntax=docker/dockerfile:1

ARG RUST_VERSION=1.91.0
ARG APP_NAME=timetracker

FROM rust:${RUST_VERSION}-alpine AS build
ARG APP_NAME
WORKDIR /app

RUN apk add --no-cache clang lld musl-dev git

RUN --mount=type=bind,source=src,target=src \
    --mount=type=bind,source=templates,target=templates \
    --mount=type=bind,source=static,target=static \
    --mount=type=bind,source=Cargo.toml,target=Cargo.toml \
    --mount=type=bind,source=Cargo.lock,target=Cargo.lock \
    --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
cargo build --locked --profile release-prod && \
cp ./target/release-prod/$APP_NAME /bin/server


FROM alpine:3.18 AS final


ARG UID=10001
RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    appuser \
    && mkdir -p /app/data \
    && chown -R appuser:appuser /app/data
USER appuser

COPY --from=build /bin/server /bin/

EXPOSE 3000

CMD ["/bin/server"]
