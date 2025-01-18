# Stage 1: Build
FROM rustlang/rust:nightly AS builder

COPY . /usr/src/app

WORKDIR /usr/src/app
RUN cargo build --release


FROM gcr.io/distroless/cc AS runtime
# Alternatively, use alpine for a more flexible lightweight image
# FROM alpine:latest

WORKDIR /usr/src/app

COPY --from=builder /usr/src/app/target/release/onebrc-rs /1brc


ENTRYPOINT ["/1brc"]
