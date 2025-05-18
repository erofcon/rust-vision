use opencv as cv;
use cv::prelude::*;
use cv::videoio::{VideoCapture, CAP_ANY};
use std::env;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use gst::prelude::*;
// Структура для контроля воспроизведения
struct PlaybackControl {
    playing: bool,
    seek_pos: Option<f64>, // Позиция для перемотки в секундах
    speed: f64,            // Скорость воспроизведения (1.0 = нормальная)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Инициализация GStreamer и OpenCV
    gst::init()?;

    // Получаем путь к файлу из аргументов командной строки
    let args: Vec<String> = env::args().collect();
    let file_path = args.get(1).map(|s| s.as_str()).unwrap_or("2.dav");

    println!("Открываем файл: {}", file_path);

    // Пробуем открыть через GStreamer напрямую
    let direct_gst = try_gstreamer_direct(file_path);

    match direct_gst {
        Ok(_) => {
            println!("Файл успешно открыт через GStreamer напрямую.");
            return Ok(());
        }
        Err(e) => {
            println!("Не удалось открыть файл через GStreamer напрямую: {}", e);
            println!("Пробуем использовать мост OpenCV -> GStreamer...");
        }
    }

    // Пробуем открыть файл через OpenCV
    let mut cap = VideoCapture::from_file(file_path, CAP_ANY)?;
    if !cap.is_opened()? {
        return Err("Не удалось открыть видеофайл ни через GStreamer, ни через OpenCV".into());
    }

    // Получаем информацию о видео
    let fps = cap.get(cv::videoio::CAP_PROP_FPS)? as f64;
    let width = cap.get(cv::videoio::CAP_PROP_FRAME_WIDTH)? as i32;
    let height = cap.get(cv::videoio::CAP_PROP_FRAME_HEIGHT)? as i32;
    let frame_count = cap.get(cv::videoio::CAP_PROP_FRAME_COUNT)? as i64;
    let duration_sec = if fps > 0.0 { frame_count as f64 / fps } else { 0.0 };

    println!("Видео: {}x{} @{:.2} FPS, кадров: {}, длительность: {:.2} сек",
             width, height, fps, frame_count, duration_sec);

    // Создаем GStreamer пайплайн для вывода
    let pipeline_str = format!(
        "appsrc name=source is-live=true format=time ! videoconvert ! videoscale ! video/x-raw,width={},height={} ! autovideosink sync=true",
        width, height
    );

    println!("Создаем пайплайн: {}", pipeline_str);

    let pipeline = gst::parse_launch(&pipeline_str)?;
    let pipeline = pipeline.dynamic_cast::<gst::Pipeline>().unwrap();

    // Получаем элемент appsrc
    let appsrc = pipeline
        .by_name("source")
        .expect("Не удалось получить элемент appsrc");
    let appsrc = appsrc.dynamic_cast::<gst_app::AppSrc>().unwrap();

    // Настраиваем appsrc
    let caps_str = format!(
        "video/x-raw, format=BGR, width={}, height={}, framerate={}/1",
        width, height, fps as i32
    );
    let caps = gst::Caps::from_string(&caps_str).unwrap();
    appsrc.set_caps(Some(&caps));
    appsrc.set_format(gst::Format::Time);
    appsrc.set_max_bytes(1_000_000); // 1MB буфер

    // Запускаем пайплайн
    pipeline.set_state(gst::State::Playing)?;

    // Создаем управление воспроизведением
    let playback_control = Arc::new(Mutex::new(PlaybackControl {
        playing: true,
        seek_pos: None,
        speed: 1.0,
    }));

    // Создаем поток для клавиатурного ввода
    let keyboard_control = playback_control.clone();
    let keyboard_thread = thread::spawn(move || {
        println!("Управление:");
        println!("  Space: Пауза/Воспроизведение");
        println!("  Left/Right: Перемотка -/+ 10 секунд");
        println!("  Up/Down: Изменение скорости воспроизведения +/- 0.25x");
        println!("  Q: Выход");

        // В реальном сценарии здесь должен быть код для обработки клавиатурного ввода
        // Для простоты этот пример не содержит фактического кода обработки клавиш
    });

    // Создаем поток для чтения кадров из OpenCV и передачи их в GStreamer
    let frame_control = playback_control.clone();
    let pipeline_weak = pipeline.downgrade();
    let appsrc_weak = appsrc.downgrade();

    let frame_thread = thread::spawn(move || {
        let mut frame = cv::core::Mat::default();
        let mut pts: u64 = 0;
        let mut last_push_time = Instant::now();
        let frame_duration_ns = if fps > 0.0 { 1_000_000_000 / fps } else { 33_333_333 }; // в наносекундах

        loop {
            // Проверяем состояние воспроизведения
            let mut control = frame_control.lock().unwrap();

            if !control.playing {
                // Если на паузе, ждем и проверяем снова
                drop(control);
                thread::sleep(Duration::from_millis(100));
                continue;
            }

            // Проверяем запрос на перемотку
            if let Some(seek_pos) = control.seek_pos.take() {
                let frame_pos = (seek_pos * fps) as i32;
                cap.set(cv::videoio::CAP_PROP_POS_FRAMES, frame_pos as f64).unwrap_or(());
                pts = (seek_pos * 1_000_000_000.0) as u64;
            }

            // Получаем скорость воспроизведения
            let speed = control.speed;
            drop(control);

            // Читаем кадр из OpenCV
            match cap.read(&mut frame) {
                Ok(true) if !frame.empty() => {
                    // Кадр успешно прочитан
                    if let Some(appsrc) = appsrc_weak.upgrade() {
                        // Получаем данные кадра
                        let data = match frame.data_bytes() {
                            Ok(data) => data,
                            Err(_) => continue,
                        };

                        // Создаем буфер GStreamer
                        let mut buffer = match gst::Buffer::with_size(data.len()) {
                            Ok(buffer) => buffer,
                            Err(_) => continue,
                        };

                        {
                            let buffer_ref = buffer.make_mut();
                            buffer_ref.set_pts(gst::ClockTime::from_nseconds(pts));
                            buffer_ref.set_duration(gst::ClockTime::from_nseconds(frame_duration_ns as u64));

                            // Копируем данные в буфер
                            if let Ok(mut map) = buffer_ref.map_writable() {
                                let buffer_data = map.as_mut_slice();
                                buffer_data.copy_from_slice(data);
                            } else {
                                continue;
                            }
                        }

                        // Отправляем буфер в пайплайн
                        if appsrc.push_buffer(buffer).is_err() {
                            break;
                        }

                        // Увеличиваем PTS для следующего кадра
                        pts += frame_duration_ns as u64;

                        // Контролируем скорость воспроизведения
                        let elapsed = last_push_time.elapsed();
                        let target_time = Duration::from_secs_f64(1.0 / (fps * speed));

                        if elapsed < target_time {
                            thread::sleep(target_time - elapsed);
                        }

                        last_push_time = Instant::now();
                    } else {
                        // appsrc больше не доступен
                        break;
                    }
                }
                Ok(false) => {
                    // Достигнут конец файла
                    println!("Достигнут конец файла");
                    if let Some(appsrc) = appsrc_weak.upgrade() {
                        let _ = appsrc.end_of_stream();
                    }
                    break;
                }
                _ => {
                    // Ошибка чтения кадра
                    println!("Ошибка чтения кадра");
                    break;
                }
            }
        }
    });

    // Обрабатываем сообщения из шины GStreamer
    let bus = pipeline.bus().unwrap();
    for msg in bus.iter_timed(gst::ClockTime::NONE) {
        match msg.view() {
            gst::MessageView::Error(err) => {
                eprintln!(
                    "Ошибка: {} ({:?})",
                    err.error(),
                    err.debug().unwrap_or_default()
                );
                break;
            }
            gst::MessageView::Eos(_) => {
                println!("Конец потока");
                break;
            }
            _ => {}
        }
    }

    // Останавливаем воспроизведение
    {
        let mut control = playback_control.lock().unwrap();
        control.playing = false;
    }

    // Останавливаем пайплайн
    pipeline.set_state(gst::State::Null)?;

    // Ожидаем завершения потоков
    let _ = frame_thread.join();
    let _ = keyboard_thread.join();

    println!("Воспроизведение завершено");

    Ok(())
}

// Пытаемся открыть файл напрямую через GStreamer
fn try_gstreamer_direct(file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Пробуем различные декодеры и демультиплексоры
    let pipeline_attempts = vec![
        format!("filesrc location={} ! qtdemux ! h264parse ! avdec_h264 ! videoconvert ! autovideosink", file_path),
        format!("filesrc location={} ! matroskademux ! h264parse ! avdec_h264 ! videoconvert ! autovideosink", file_path),
        format!("filesrc location={} ! tsdemux ! h264parse ! avdec_h264 ! videoconvert ! autovideosink", file_path),
        format!("filesrc location={} ! avidemux ! h264parse ! avdec_h264 ! videoconvert ! autovideosink", file_path),
        format!("filesrc location={} ! decodebin3 ! videoconvert ! autovideosink", file_path),
    ];

    for (i, pipeline_str) in pipeline_attempts.iter().enumerate() {
        println!("Попытка {}/{}: {}", i+1, pipeline_attempts.len(), pipeline_str);

        match gst::parse_launch(pipeline_str) {
            Ok(element) => {
                let pipeline = element.dynamic_cast::<gst::Pipeline>().unwrap();

                // Запускаем пайплайн
                if pipeline.set_state(gst::State::Playing).is_err() {
                    println!("Не удалось запустить пайплайн");
                    continue;
                }

                // Ждем, пока пайплайн перейдет в состояние Playing или выдаст ошибку
                let timeout = gst::ClockTime::from_seconds(5);
                match pipeline.state(timeout) {
                    (gst::StateChangeReturn::Success, gst::State::Playing, _) => {
                        // Успешно запущен
                        println!("Пайплайн успешно запущен! Воспроизводим...");

                        // Обрабатываем сообщения из шины GStreamer
                        let bus = pipeline.bus().unwrap();
                        for msg in bus.iter_timed(gst::ClockTime::NONE) {
                            match msg.view() {
                                gst::MessageView::Error(err) => {
                                    println!(
                                        "Ошибка: {} ({:?})",
                                        err.error(),
                                        err.debug().unwrap_or_default()
                                    );
                                    pipeline.set_state(gst::State::Null)?;
                                    break;
                                }
                                gst::MessageView::Eos(_) => {
                                    println!("Конец потока");
                                    pipeline.set_state(gst::State::Null)?;
                                    return Ok(());
                                }
                                _ => {}
                            }
                        }

                        pipeline.set_state(gst::State::Null)?;
                        return Ok(());
                    }
                    _ => {
                        // Не удалось запустить в течение таймаута
                        println!("Не удалось перевести пайплайн в состояние Playing");
                        pipeline.set_state(gst::State::Null)?;
                        continue;
                    }
                }
            }
            Err(e) => {
                println!("Ошибка создания пайплайна: {}", e);
                continue;
            }
        }
    }

    // Если ни один из методов не сработал
    Err("Не удалось открыть файл через GStreamer напрямую".into())
}