

use std::time::Duration;

use deadpool_lapin::{lapin::{options::{BasicAckOptions, BasicConsumeOptions}, ConsumerDelegate}, Object};
use deadpool_lapin::Pool;
use tokio_stream::StreamExt;
use crate::{mint::{mint_nft, ActionType, MintResult, NotifData, UpdateUserNftRequest}, producer::ProduceThis, *};
use serde::{Serialize, Deserialize};



#[derive(Message, Clone, Serialize, Deserialize, Debug, Default)]
#[rtype(result = "()")]
pub struct GetMessage{
    pub queue: String,
    pub exchange_name: String,
    pub routing_key: String,
    pub tag: String,
}


#[derive(Clone)]
pub struct ConsumerActor{
    pub pool: deadpool_lapin::Pool,
    pub producer_actor: Addr<ProducerActor>
}


impl Actor for ConsumerActor{
    type Context = Context<Self>;
    fn started(&mut self, ctx: &mut Self::Context) {

        log::info!("start consuming");
        ctx.run_interval(Duration::from_secs(5), |actor, ctx|{

            

        });
    }
}

impl ConsumerActor{

    pub fn new(pool: deadpool_lapin::Pool, producer_actor: Addr<ProducerActor>) -> Self{
        Self { pool, producer_actor }
    }

    pub async fn consume(&self,
        consumer_tag: &str, queue: &str, 
        routing_key: &str, exchange: &str
    ){
    
            let pool = self.pool.clone();
            let producer_actor = self.producer_actor.clone();

            let conn = pool.get().await.unwrap();
            let create_channel = conn.create_channel().await;
            match create_channel{
                Ok(chan) => {

                    // -ˋˏ✄┈┈┈┈ making a queue inside the broker per each consumer, 
                    let create_queue = chan
                        .queue_declare(
                            &queue,
                            QueueDeclareOptions::default(),
                            FieldTable::default(),
                        )
                        .await;

                    let Ok(q) = create_queue else{
                        let e = create_queue.unwrap_err();
                        log::error!("error due to: {:?}", e);
                        return;
                    };

                    // binding the queue to the exchange routing key
                    /* -ˋˏ✄┈┈┈┈ 
                        if the exchange is not direct or is fanout or topic we should bind the 
                        queue to the exchange to consume the messages from the queue. binding 
                        the queue to the passed in exchange, if the exchange is direct every 
                        queue that is created is automatically bounded to it with a routing key 
                        which is the same as the queue name, the direct exchange is "" and 
                        rmq doesn't allow to bind any queue to that manually
                    */
                    if exchange != ""{ // it's either fanout, topic or headers
                        match chan
                            .queue_bind(q.name().as_str(), &exchange, &routing_key, 
                                QueueBindOptions::default(), FieldTable::default()
                            )
                            .await
                            {
                                Ok(_) => {},
                                Err(e) => {
                                    log::error!("error due to: {:?}", e);
                                    return;
                                }
                            }
                    }

                    // since &str is not lived long enough to be passed to the tokio spawn
                    // if it was static it would be good however we're converting them to
                    // String to pass the String version of them to the tokio spawn scope
                    let cloned_consumer_tag = consumer_tag.to_string();
                    let cloned_queue = queue.to_string();
                    tokio::spawn(async move{

                        // -ˋˏ✄┈┈┈┈ consuming from the queue owned by this consumer
                        match chan
                            .basic_consume(
                                // the queue that is bounded to the exchange to receive messages based on the routing key
                                // since the queue is already bounded to the exchange and its routing key it only receives 
                                // messages from the exchange that matches and follows the passed in routing pattern like:
                                // message routing key "orders.processed" might match a binding with routing key "orders.#
                                // if none the messages follow the pattern then the queue will receive no message from the 
                                // exchange based on that pattern!
                                &cloned_queue, 
                                &cloned_consumer_tag, // custom consumer name
                                BasicConsumeOptions::default(), 
                                FieldTable::default()
                            )
                            .await
                        {
                            Ok(mut consumer) => {

                                // stream over consumer to receive data from the queue
                                while let Some(delivery) = consumer.next().await{
                                    match delivery{
                                        Ok(delv) => {

                                            // if the consumer receives this data from the queue
                                            match delv.ack(BasicAckOptions::default()).await{
                                                Ok(ok) => {

                                                    let buffer = delv.data;
                                                    let data = std::str::from_utf8(&buffer).unwrap();

                                                    let notif_data = serde_json::from_slice::<NotifData>(&buffer).unwrap();
                                                    let mint_nft_request = serde_json::from_value::<UpdateUserNftRequest>(notif_data.action_data.unwrap()).unwrap();
                                                    log::info!("received data from rmq > {:?}", mint_nft_request);

                                                    let cloned_mint_nft_request = mint_nft_request.clone();
                                                    let (respmint_sender, mut respmint_receiver) = tokio::sync::mpsc::channel::<(String, String, u8)>(1024);
                                                    
                                                    // ------ minting process in the background
                                                    tokio::spawn(async move{
                                                        let (new_tx_hash, token_id, status) = 
                                                            mint_nft(
                                                                cloned_mint_nft_request.clone()
                                                            ).await;
                                                        
                                                        respmint_sender.send((new_tx_hash, token_id, status)).await;
                                                    });
                                                    // --------

                                                    while let Some((tx_hash, token_id, status)) = respmint_receiver.recv().await{

                                                        let resp = MintResult{
                                                            nft_data: mint_nft_request.clone(),
                                                            tx_hash, 
                                                            token_id,
                                                            status,
                                                        };
                                                        let payload = serde_json::to_value(&resp).unwrap();
                                                        let notif = NotifData{ 
                                                            receiver_info: Some(serde_json::to_value(String::from("main-service")).unwrap()),
                                                            id: uuid::Uuid::new_v4().to_string(), 
                                                            action_data: Some(serde_json::to_value(&resp).unwrap()), 
                                                            actioner_info: Some(serde_json::to_value(&String::from("mints-producer")).unwrap()), 
                                                            action_type: ActionType::Mint, 
                                                            fired_at: Some(chrono::Local::now().timestamp()), 
                                                            is_seen: false 
                                                        };
                                                        
                                                        let payload = serde_json::to_string(&notif).unwrap();

                                                        // if we use a bridge that is overhead cause everytime a consumer needs a producer 
                                                        // it must talk to the bridge to get a new instance of that which faces us memory 
                                                        // overhead at runtime
                                                        // send minted nft to producer actor
                                                        tokio::spawn(
                                                            {
                                                                let cloned_payload = payload.clone();
                                                                let cloned_producer_actor = producer_actor.clone();
                                                                let ex = std::env::var("EXCHANGE").unwrap();
                                                                let r = std::env::var("ROUTING_KEY").unwrap();
                                                                let et = std::env::var("EXCHANGE_TYPE").unwrap();
                                                                async move{
                                                                    cloned_producer_actor.send(ProduceThis{
                                                                        data: cloned_payload,
                                                                        exchange_name: ex,
                                                                        exchange_type: r,
                                                                        routing_key: et,
                                                                    }).await;
                                                                }
                                                            }
                                                        );
                                                    }
                                                    
                                                },
                                                Err(e) => {
                                                    log::error!("error due to: {:?}", e);
                                                    return;
                                                }
                                            }
                
                                        },
                                        Err(e) => {
                                            log::error!("error due to: {:?}", e);
                                            return;
                                        }
                                    }
                                }
                            },
                            Err(e) => {
                                log::error!("error due to: {:?}", e);
                                return;
                            }
                        }

                    });

                },
                Err(e) => {
                    log::error!("error due to: {:?}", e);
                    return;   
                }
            }
    
    
    }

}

impl Handler<GetMessage> for ConsumerActor{
    type Result = ();
    fn handle(&mut self, msg: GetMessage, ctx: &mut Self::Context) -> Self::Result {
        let GetMessage{tag, queue, routing_key, exchange_name} = msg.clone();

        let this = self.clone();
        // tokio::spawn(async move{
        //     this.consume(&tag, &queue, &routing_key, &exchange_name).await;
        // });

        async move{
            this.consume(&tag, &queue, &routing_key, &exchange_name).await;
        }.into_actor(self)
        .spawn(ctx);

    }
}