use std::thread;
use std::time::Duration;
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use anyhow::Result;
use futures_util::StreamExt;
use lapin::options::{
    BasicAckOptions, BasicConsumeOptions, BasicPublishOptions, QueueDeclareOptions,
};
use lapin::types::FieldTable;
use lapin::{BasicProperties, Connection, ConnectionProperties};

#[get("/get_tasks")]
async fn get_tasks() -> impl Responder {
    HttpResponse::Ok().body("get_tasks")
}

#[get("/get_tasks_status")]
async fn get_tasks_status() -> impl Responder {
    HttpResponse::Ok().body("get_tasks_status")
}

#[post("/add_task")]
async fn add_task() -> impl Responder {
    HttpResponse::Ok().body("add_task")
}

#[actix_web::main]
async fn main() -> Result<()> {
    // let addr = "amqp://127.0.0.1:5672";
    // let conn = Connection::connect(addr, ConnectionProperties::default()).await?;
    // let channel = conn.create_channel().await?;
    //
    // channel
    //     .queue_declare(
    //         "task_queue",
    //         QueueDeclareOptions::default(),
    //         FieldTable::default(),
    //     )
    //     .await?;
    //
    // let mut consumer = channel
    //     .basic_consume(
    //         "task_queue",
    //         "consumer",
    //         BasicConsumeOptions::default(),
    //         FieldTable::default(),
    //     )
    //     .await
    //     .expect("basic_consume");
    //
    // println!(" [*] Waiting for messages. To exit press CTRL+C");
    //
    // while let Some(delivery) = consumer.next().await {
    //     if let Ok(delivery) = delivery {
    //         println!(" [x] Received {:?}", std::str::from_utf8(&delivery.data)?);
    //         thread::sleep(Duration::from_secs(delivery.data.len() as u64));
    //         println!(" [x] Done");
    //         delivery.ack(BasicAckOptions::default())
    //             .await
    //             .expect("basic_ack");
    //     }
    // }

    // let args: Vec<_> = std::env::args().skip(1).collect();
    // let message = match args.len() {
    //     0 => "hello".to_string(),
    //     _ => args.join(" ").to_string(),
    // };
    //
    // let addr = "amqp://127.0.0.1:5672";
    // let conn = Connection::connect(addr, ConnectionProperties::default()).await?;
    // let channel = conn.create_channel().await?;
    //
    // channel
    //     .queue_declare(
    //         "task_queue",
    //         QueueDeclareOptions::default(),
    //         FieldTable::default(),
    //     )
    //     .await?;
    //
    // channel
    //     .basic_publish(
    //         "",
    //         "task_queue",
    //         BasicPublishOptions::default(),
    //         message.as_bytes(),
    //         BasicProperties::default(),
    //     )
    //     .await?;
    //
    // println!(" [x] Sent {:?}", std::str::from_utf8(message.as_bytes())?);
    //
    // conn.close(0, "").await?;


    // // let payload = "Hello world!".as_bytes();
    // //
    // // channel
    // //     .basic_publish(
    // //         "",
    // //         "hello",
    // //         BasicPublishOptions::default(),
    // //         payload,
    // //         BasicProperties::default(),
    // //     )
    // //     .await?;
    // //
    // // println!(" [x] Sent \"Hello World!\"");
    //
    // conn.close(0, "").await?;

    // HttpServer::new(|| {
    //     App::new().service(web::scope("/api/v1").service(get_tasks).service(add_task))
    // })
    // .bind("127.0.0.1:8080")
    // .unwrap()
    // .run()
    // .await?;

    Ok(())
}
