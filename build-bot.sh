#!/bin/sh

DOCKER_BUILDKIT=1 docker build --network host -t "auction-bot" -o bin .
scp bin/agsol-gold-bot root@95.217.237.232:~/auction-bot/deploy
