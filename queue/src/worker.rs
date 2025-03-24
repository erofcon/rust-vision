use anyhow::Result;
use async_trait::async_trait;
use futures_util::StreamExt;
use lapin::options::{BasicAckOptions, BasicConsumeOptions, BasicNackOptions, QueueDeclareOptions};
use lapin::types::FieldTable;
use lapin::{Channel, Connection, ConnectionProperties, Consumer, Queue};
use std::sync::Arc;

/// Обработчик задач, реализуемый пользователем библиотеки
#[async_trait]
pub trait JobHandler: Send + Sync + 'static {
    /// Метод для обработки полученных задач
    ///
    /// # Параметры
    /// * `payload` - Тело сообщения в виде байтового массива
    ///
    /// # Возвращает
    /// * `Result<(), Box<dyn std::error::Error>>` - Результат обработки
    async fn handle_job(
        &self,
        payload: &[u8],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Конфигурация для воркера
pub struct WorkerConfig {
    /// URL подключения к RabbitMQ (например, "amqp://guest:guest@localhost:5672")
    pub url: String,
    /// Имя очереди для прослушивания
    pub queue_name: String,
    /// Количество сообщений, которое можно получить до отправки подтверждения
    pub prefetch_count: u16,
    /// Тег для идентификации потребителя
    pub consumer_tag: String,
}

/// Основной класс воркера для обработки задач из RabbitMQ
pub struct Worker<H: JobHandler> {
    config: WorkerConfig,
    connection: Connection,
    channel: Channel,
    queue: Queue,
    handler: Arc<H>,
    consumer: Option<Consumer>,
}

impl<H: JobHandler> Worker<H> {
    /// Создает новый экземпляр воркера
    ///
    /// # Параметры
    /// * `config` - Конфигурация для воркера
    /// * `handler` - Обработчик задач
    ///
    /// # Возвращает
    /// * `Result<Self, Box<dyn std::error::Error>>` - Результат создания воркера
    pub async fn new(config: WorkerConfig, handler: H) -> Result<Self, Box<dyn std::error::Error>> {
        let connection = Connection::connect(&config.url, ConnectionProperties::default()).await?;
        let channel = connection.create_channel().await?;

        // Устанавливаем prefetch для канала
        channel
            .basic_qos(
                config.prefetch_count,
                lapin::options::BasicQosOptions::default(),
            )
            .await?;

        let queue = channel
            .queue_declare(
                &config.queue_name,
                QueueDeclareOptions {
                    durable: true,
                    ..QueueDeclareOptions::default()
                },
                FieldTable::default(),
            )
            .await?;

        Ok(Self {
            config,
            connection,
            channel,
            queue,
            handler: Arc::new(handler),
            consumer: None,
        })
    }

    /// Запускает воркер и начинает обработку сообщений
    ///
    /// # Возвращает
    /// * `Result<(), Box<dyn std::error::Error>>` - Результат запуска воркера
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Запуск воркера для очереди: {}", self.config.queue_name);

        let mut consumer = self
            .channel
            .basic_consume(
                &self.config.queue_name,
                &self.config.consumer_tag,
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;

        let channel = self.channel.clone();
        let handler = self.handler.clone();

        while let Some(delivery) = consumer.next().await {
            match delivery {
                Ok(delivery) => {
                    let delivery_tag = delivery.delivery_tag;

                    let payload = delivery.data.to_vec();
                    let channel_clone = channel.clone();
                    let handler_clone = handler.clone();

                    tokio::spawn(async move {
                        match handler_clone.handle_job(&payload).await {
                            Ok(_) => {
                                // Успешная обработка - подтверждаем сообщение
                                if let Err(e) = channel_clone
                                    .basic_ack(delivery_tag, BasicAckOptions::default())
                                    .await
                                {
                                    eprintln!("Ошибка при подтверждении сообщения: {}", e);
                                }
                            }
                            Err(e) => {
                                // Ошибка обработки - отклоняем сообщение с requeue=true
                                eprintln!("Ошибка при обработке сообщения: {}", e);
                                if let Err(e) = channel_clone
                                    .basic_nack(
                                        delivery_tag,
                                        BasicNackOptions {
                                            requeue: true,
                                            ..BasicNackOptions::default()
                                        },
                                    )
                                    .await
                                {
                                    eprintln!("Ошибка при отклонении сообщения: {}", e);
                                }
                            }
                        }
                    });
                }
                Err(e) => {
                    eprintln!("Ошибка при получении сообщения: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Останавливает воркер
    ///
    /// # Возвращает
    /// * `Result<(), Box<dyn std::error::Error>>` - Результат остановки воркера
    pub async fn stop(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Остановка воркера для очереди: {}", self.config.queue_name);
        self.channel.close(0, "Нормальное завершение").await?;
        self.connection.close(0, "Нормальное завершение").await?;
        Ok(())
    }
}
