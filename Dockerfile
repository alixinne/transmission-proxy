FROM docker.io/library/rust:1.73.0 AS build

WORKDIR /src

# Build full project
COPY . .
RUN \
    --mount=type=cache,target=/src/target/release/build \
    --mount=type=cache,target=/src/target/release/deps \
    --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release

FROM gcr.io/distroless/cc-debian12@sha256:66d87e170bc2c5e2b8cf853501141c3c55b4e502b8677595c57534df54a68cc5
COPY --from=build /src/target/release/transmission-proxy /
COPY public /public
CMD ["/transmission-proxy", "--bind", "http://0.0.0.0:3000/transmission", "--serve-root", "/public"]
EXPOSE 3000/tcp
