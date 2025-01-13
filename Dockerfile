FROM docker.io/library/rust:1.73.0 AS build

WORKDIR /src

# Build full project
COPY . .
RUN \
    --mount=type=cache,target=/src/target/release/build \
    --mount=type=cache,target=/src/target/release/deps \
    --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release

FROM gcr.io/distroless/cc-debian12@sha256:b7550f0b15838de14c564337eef2b804ba593ae55d81ca855421bd52f19bb480
COPY --from=build /src/target/release/transmission-proxy /
COPY public /public
CMD ["/transmission-proxy", "--bind", "http://0.0.0.0:3000/transmission", "--serve-root", "/public"]
EXPOSE 3000/tcp
