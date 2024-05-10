



NFT mint service actor worker over RMQ

### flow?

```
step1) in main service: send product info to notif producer actor to send to rmq
step2) in mint service: 
    notif consumer actor receives the product info and starts the minting process
    once it finishes with the process the result will be sent to rmq using its
    notif producer actor
step3) in main service: 
    notif consumer actor begins to start consuming in the background it can be either
    where the http server is being started, by callig an api to register it or inside
    a loop{} to keep the app running constantly to receive messages but in either way
    we know the queue! as soon as its actor gets started, it receives all products 
    constantly from the rmq, if the product was minted or there was any error then we 
    release the id from the locker
```

> your NFT data structure must be as the followings:

```rust
pub struct UpdateUserNftRequest{
    pub caller_cid: String,
    pub col_id: i32,
    pub buyer_screen_cid: Option<String>,
    pub transfer_to_screen_cid: Option<String>,
    pub amount: i64, // amount of gas fee for this call
    pub nft_id: i32,
    pub event_type: String,
    pub contract_address: String,
    pub current_owner_screen_cid: String,
    pub metadata_uri: String,
    pub extra: Option<serde_json::Value>,
    pub attributes: Option<serde_json::Value>,
    pub onchain_id: Option<String>, 
    pub nft_name: String,
    pub is_minted: Option<bool>,
    pub nft_description: String,
    pub current_price: Option<i64>,
    pub is_listed: Option<bool>,
    pub freeze_metadata: Option<bool>,
    pub comments: Option<serde_json::Value>,
    pub likes: Option<serde_json::Value>,
    pub tx_hash: Option<String>,
    pub tx_signature: String,
    pub hash_data: String,
}
```

### how 2?

> make sure yo've updated the `NFTPORT_TOKEN` and `AMQP_PASSWORD` variables with your own values.

```bash
TIMESTAMP=$(date +%s)
sudo docker build -t mints-actor-worker-$TIMESTAMP -f . --no-cache
sudo docker run -d --restart unless-stopped --network hoopoe --name mints-actor-worker-$TIMESTAMP mints-actor-worker-$TIMESTAMP

sudo docker run -d --network hoopoe --hostname rabbitmq -p 5672:5672 -p 15672:15672 --name rabbitmq -e RABBITMQ_DEFAULT_USER=hoopoe -e RABBITMQ_DEFAULT_PASS=geDteDd0Ltg2135FJYQ6rjNYHYkGQa70 rabbitmq:3-management
```