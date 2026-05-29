FROM docker.io/library/rust:1.96.0 AS build

WORKDIR /src

# Build full project
COPY . .
RUN \
    --mount=type=cache,target=/src/target/release/build \
    --mount=type=cache,target=/src/target/release/deps \
    --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release

FROM gcr.io/distroless/cc-debian12@sha256:aa0b7af67fa8211751ea6e00baa8373ba56cc1417ffc986ec9619bd0e1556b56
COPY --from=build /src/target/release/transmission-proxy /
COPY public /public
CMD ["/transmission-proxy", "--bind", "http://0.0.0.0:3000/transmission", "--serve-root", "/public"]
EXPOSE 3000/tcp
