FROM rust:1.58.0 as builder

WORKDIR /usr/src/auction-bot
COPY . .
RUN apt-get update && apt-get install libudev-dev && rm -rf /var/lib/apt/lists/*
RUN cargo build --example auction_bot --release
