docker build --network host -t "auction-bot" .
docker cp <container>:/usr/src/auction-bot/target/release/examples/agsol-gold-bot .
scp agsol-gold-bot root@95.217.237.232:~/auction_bot
rm agsol-gold-bot
