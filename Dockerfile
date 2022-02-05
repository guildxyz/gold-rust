FROM rust:1.58.0 AS build-stage

WORKDIR /usr/src/auction-bot
COPY . .
RUN apt-get update && apt-get install libudev-dev && rm -rf /var/lib/apt/lists/*
RUN cargo build --package agsol-gold-bot --release

FROM scratch AS export-stage
COPY --from=build-stage /usr/src/auction-bot/target/release/agsol-gold-bot .
