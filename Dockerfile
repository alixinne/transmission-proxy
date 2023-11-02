FROM docker.io/library/rust:1.73.0 AS build

WORKDIR /src

# Build full project
COPY . .
RUN \
    --mount=type=cache,target=/src/target/release/build \
    --mount=type=cache,target=/src/target/release/deps \
    --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release

FROM gcr.io/distroless/cc@sha256:b82f113425c5b5c714151aaacd8039bc141821cdcd3c65202d42bdf9c43ae60b
COPY --from=build /src/target/release/transmission-proxy /
COPY public /public
CMD ["/transmission-proxy", "--bind", "http://0.0.0.0:3000/transmission", "--serve-root", "/public"]
EXPOSE 3000/tcp
