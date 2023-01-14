FROM docker.io/library/rust:1.62.0 AS build

WORKDIR /src

# Prebuild dependencies
RUN cargo init \
 && mkdir .cargo

COPY Cargo.lock .
COPY Cargo.toml .
RUN cargo vendor > .cargo/config \
 && cargo build --release

# Build full project
COPY . /src
RUN cargo build --release

FROM gcr.io/distroless/cc@sha256:101c26286ea36b68200ff94cf95ca9dbde3329c987738cba3ba702efa3465f6f
COPY --from=build /src/target/release/transmission-proxy /
COPY public /public
CMD ["/transmission-proxy", "--bind", "http://0.0.0.0:3000/transmission", "--serve-root", "/public"]
EXPOSE 3000/tcp
