docker build -t "auction-bot" .
docker cp auction-bot:latest/usr/src/auction-bot/target/release/examples/auction_bot .
scp auction_bot root@95.217.237.232:~/auction_bot
