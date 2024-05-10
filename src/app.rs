



mod mint;
mod producer;
mod consumer;

use crate::producer::ProducerActor;
use consumer::{GetMessage, ConsumerActor};
use deadpool_lapin::{Config, Manager, Pool as LapinDeadPool, Runtime};
use deadpool_lapin::lapin::{
    options::BasicPublishOptions,
    BasicProperties,
};
use deadpool_lapin::lapin::options::{ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions};
use deadpool_lapin::lapin::protocol::exchange;
use deadpool_lapin::lapin::types::FieldTable;
use actix::prelude::*;
use actix::{AsyncContext, Context};
use actix_redis::{resp_array, Command, RespValue};
use env_logger::Env;
use mint::{MintResult, UpdateUserNftRequest};
use producer::ProduceThis;
use std::sync::Arc;
use dotenv::dotenv;
use actix::System;
use actix_web::rt::System as RtSystem;


// don't run actor in the context of tokio runtime, use actix
#[actix_web::main]
async fn main() -> std::io::Result<()>{

    dotenv::dotenv().expect("expected .env file be there!");
    env_logger::init_from_env(Env::default().default_filter_or("info"));
    std::env::set_var("RUST_LOG", "trace");

    let rmq_port = std::env::var("AMQP_PORT").unwrap();
    let rmq_host = std::env::var("AMQP_HOST").unwrap();
    let rmq_username = std::env::var("AMQP_USERNAME").unwrap();
    let rmq_password = std::env::var("AMQP_PASSWORD").unwrap();
    let rmq_addr = format!("amqp://{}:{}@{}:{}", rmq_username, rmq_password, rmq_host, rmq_port);
    let mut cfg = Config::default();
    cfg.url = Some(rmq_addr);
    let lapin_pool = cfg.create_pool(Some(Runtime::Tokio1)).unwrap();

    let producer_actor = ProducerActor::new(lapin_pool.clone()).start();
    let consumer_actor = ConsumerActor::new(lapin_pool.clone(), producer_actor.clone()).start();

    // keep the app running constantly
    loop{
        // let consumer_actor = consumer_actor.clone();
        // tokio::spawn(async move{
            consumer_actor.send(
                GetMessage{ 
                    tag: std::env::var("CONSUMER_TAG").unwrap(), 
                    queue: std::env::var("MINT_QUEUE").unwrap(), 
                    routing_key: std::env::var("ROUTING_KEY").unwrap(), 
                    exchange_name: std::env::var("EXCHANGE").unwrap()
                }
            ).await;
        // });
    }


}