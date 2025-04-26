use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::oneshot;
use uuid::Uuid;

// Структура для информации об остановке
struct StopInfo {
    flag: Arc<AtomicBool>,
    cancel_sender: Option<oneshot::Sender<()>>,
}

// Менеджер обработки видео (thread-safe)
pub struct VideoProcessingManager {
    processing_videos: Mutex<HashMap<Uuid, StopInfo>>,
}

impl VideoProcessingManager {
    pub fn new() -> Self {
        VideoProcessingManager {
            processing_videos: Mutex::new(HashMap::new()),
        }
    }

    // Регистрация нового видео в обработке
    pub fn register_video(
        &self,
        video_id: Uuid,
        stop_flag: Arc<AtomicBool>,
    ) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();

        let mut videos = self.processing_videos.lock().unwrap_or_else(|poisoned| {
            // Обработка poisoned mutex
            println!("Внимание: обнаружен poisoned mutex при регистрации видео!");
            poisoned.into_inner()
        });

        videos.insert(
            video_id,
            StopInfo {
                flag: stop_flag,
                cancel_sender: Some(tx),
            },
        );

        rx
    }

    // Остановка обработки видео
    pub fn stop_video(&self, video_id: &Uuid) -> bool {
        let mut videos = self.processing_videos.lock().unwrap_or_else(|poisoned| {
            println!("Внимание: обнаружен poisoned mutex при остановке видео!");
            poisoned.into_inner()
        });

        if let Some(stop_info) = videos.get_mut(video_id) {
            // Устанавливаем флаг остановки
            stop_info.flag.store(true, Ordering::SeqCst);

            // Отправляем сигнал через канал для немедленного завершения обработчика
            if let Some(sender) = stop_info.cancel_sender.take() {
                let _ = sender.send(());
            }

            true
        } else {
            false
        }
    }

    // Удаление видео из списка обрабатываемых
    pub fn unregister_video(&self, video_id: &Uuid) {
        let mut videos = self.processing_videos.lock().unwrap_or_else(|poisoned| {
            println!("Внимание: обнаружен poisoned mutex при удалении видео!");
            poisoned.into_inner()
        });

        videos.remove(video_id);
    }
}
