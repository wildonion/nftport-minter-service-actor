
use deadpool_lapin::{lapin, Pool};
use crate::*;
use self::mint::MintResult;
use actix::prelude::*;


#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct ProduceThis{
    pub data: String,
    pub exchange_name: String,
    pub exchange_type: String,
    pub routing_key: String,
}

#[derive(Clone)]
pub struct ProducerActor{
    pub pool: deadpool_lapin::Pool
}

impl ProducerActor{

    pub fn new(pool: deadpool_lapin::Pool) -> Self{
        Self { pool }
    }

    pub async fn produce(&self, data: &str, exchange: &str, routing_key: &str, exchange_type: &str){
    
        // these are must be converted into String first to make longer lifetime 
        // cause &str can't get moved into tokio spawn as its lifetime it's not 
        // static the tokio spawn lives longer than the &str and the &str gets 
        // dropped out of the ram once the function is finished with executing
        let pool = self.pool.clone();
        let exchange = exchange.to_string();
        let routing_key = routing_key.to_string();
        let exchange_type = exchange_type.to_string();
        let data = data.to_string();
    
        tokio::spawn(async move{
    
            let conn = pool.get().await.unwrap();
            
            // -ˋˏ✄┈┈┈┈ creating a channel in this thread
            match conn.create_channel().await{
                Ok(chan) => {

                    // -ˋˏ✄┈┈┈┈ creating exchange
                    match chan
                        .exchange_declare(&exchange, {
                            match exchange_type.as_str(){
                                "fanout" => lapin::ExchangeKind::Fanout,
                                "direct" => lapin::ExchangeKind::Direct,
                                "headers" => lapin::ExchangeKind::Headers,
                                _ => lapin::ExchangeKind::Topic,
                            }
                        }, 
                            ExchangeDeclareOptions::default(), FieldTable::default()
                        )
                        .await
                        {
                            Ok(ex) => ex,
                            Err(e) => {
                                log::error!("error due to: {:?}", e);
                                return;
                            }

                        };

                    tokio::spawn(async move{

                        // -ˋˏ✄┈┈┈┈ publishing to exchange from this channel,
                        // later consumer bind its queue to this exchange and its
                        // routing key so messages go inside its queue, later they 
                        // can be consumed from the queue by the consumer
                        use lapin::options::BasicPublishOptions;
                        let payload = data.as_bytes();
                        match chan
                            .basic_publish(
                                &exchange, // the way of sending messages
                                &routing_key, // the way that message gets routed to the queue based on a unique routing key
                                BasicPublishOptions::default(),
                                payload, // this is the ProduceNotif data,
                                BasicProperties::default(),
                            )
                            .await
                            {
                                Ok(pc) => {
                                    let get_confirmation = pc.await;
                                    let Ok(confirmation) = get_confirmation else{
                                        let e_string = get_confirmation.unwrap_err();
                                        log::error!("error due to: {:?}", e_string.to_string());
                                        return;
                                    };

                                    if confirmation.is_ack(){
                                        log::info!("publisher sent data");
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
    
    
            
        });
        
    
    }

}

impl Actor for ProducerActor{
    type Context = Context<Self>;
    fn started(&mut self, ctx: &mut Self::Context) {
        
    }
}

impl Handler<ProduceThis> for ProducerActor{
    type Result = ();
    fn handle(&mut self, msg: ProduceThis, ctx: &mut Self::Context) -> Self::Result {

        let ProduceThis{data, exchange_name, exchange_type, routing_key} = msg.clone();

        let this = self.clone();

        tokio::spawn(async move{
            this.produce(&data, &exchange_name, &routing_key, &exchange_type).await;
        });
    }
}