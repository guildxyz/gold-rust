FROM rust:1.58.0 AS build-stage

WORKDIR /usr/src/auction-bot

RUN apt-get update \
  && apt-get install libudev-dev ca-certificates --no-install-recommends -y \
  && update-ca-certificates \
  && rm -rf /var/lib/apt/lists/* 

COPY . .
RUN cargo build --package agsol-gold-bot --release

FROM ubuntu:21.10 AS app

ARG COMMIT \
    BRANCH 
    
ENV COMMIT_SHA=${COMMIT} \
    COMMIT_BRANCH=${BRANCH}

COPY --from=build-stage /usr/src/auction-bot/target/release/agsol-gold-bot ./
COPY --from=build-stage /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

CMD ./agsol-gold-bot -d --keypair bot.json
