use std::env;
use std::io::{BufReader, Read};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use gst::parse::launch;
use gst::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Инициализация GStreamer
    gst::init()?;

    // Получаем путь к файлу из аргументов командной строки или используем по умолчанию
    let args: Vec<String> = env::args().collect();
    let file_path = args.get(1).map(|s| s.as_str()).unwrap_or("D:/Videos/2.dav");

    println!("Анализ файла с помощью ffmpeg: {}", file_path);

    // Запускаем ffmpeg для получения информации о файле
    let probe_output = Command::new("ffmpeg")
        .args(&["-i", file_path])
        .output()
        .expect("Не удалось запустить ffmpeg");

    // Обычно ffmpeg выводит информацию в stderr
    let probe_info = String::from_utf8_lossy(&probe_output.stderr);
    println!("Информация о файле:\n{}", probe_info);

    // Попытаемся извлечь информацию о разрешении и частоте кадров
    let mut width = 1280; // значения по умолчанию
    let mut height = 720;
    let mut fps = 25.0;

    // Парсинг информации о видео
    for line in probe_info.lines() {
        if line.contains("Video:") {
            // Строка может выглядеть примерно так: "Stream #0:0: Video: h264, yuv420p, 1920x1080 [SAR 1:1 DAR 16:9], 25 fps..."
            if let Some(res_pos) = line.find('x') {
                if let Some(width_start) = line[..res_pos].rfind(' ') {
                    if let Ok(w) = line[width_start + 1..res_pos].parse::<i32>() {
                        width = w;
                    }

                    let height_end = line[res_pos + 1..]
                        .find(|c: char| !c.is_digit(10))
                        .unwrap_or(line[res_pos + 1..].len());
                    if let Ok(h) = line[res_pos + 1..res_pos + 1 + height_end].parse::<i32>() {
                        height = h;
                    }
                }
            }

            if let Some(fps_pos) = line.find("fps") {
                let fps_text = &line[..fps_pos];
                if let Some(fps_start) = fps_text.rfind(' ') {
                    if let Ok(f) = fps_text[fps_start + 1..].parse::<f64>() {
                        fps = f;
                    }
                }
            }
        }
    }

    println!("Обнаружено видео {}x{} @ {} fps", width, height, fps);

    // Создаем пайплайн GStreamer для приема данных
    let pipeline_str = format!(
        "appsrc name=source is-live=true format=time caps=video/x-raw,format=I420,width={},height={},framerate={}/1 ! videoconvert ! autovideosink",
        width,
        height,
        (fps as i32)
    );

    println!("Запуск пайплайна GStreamer: {}", pipeline_str);

    let pipeline = launch(&pipeline_str)?;
    let pipeline = pipeline.dynamic_cast::<gst::Pipeline>().unwrap();

    // Получаем элемент appsrc
    let appsrc = pipeline
        .by_name("source")
        .expect("Не удалось получить элемент appsrc");
    let appsrc = appsrc.dynamic_cast::<gst_app::AppSrc>().unwrap();

    // Устанавливаем формат времени
    appsrc.set_format(gst::Format::Time);

    // Запускаем пайплайн
    pipeline.set_state(gst::State::Playing)?;

    // Запускаем ffmpeg для декодирования DAV-файла в необработанные кадры
    let ffmpeg_args = vec![
        "-i", file_path, "-f", "rawvideo", "-pix_fmt", "yuv420p", "-vsync", "0", "-",
    ];

    println!(
        "Запуск ffmpeg для декодирования: ffmpeg {}",
        ffmpeg_args.join(" ")
    );

    let ffmpeg_process = Command::new("ffmpeg")
        .args(&ffmpeg_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null()) // Игнорируем stderr для чистоты вывода
        .spawn()
        .expect("Не удалось запустить ffmpeg");

    let stdout = ffmpeg_process.stdout.expect("Не удалось получить stdout");
    let mut reader = BufReader::new(stdout);

    // Размер одного кадра в байтах (формат I420/YUV420P)
    let frame_size = width * height * 3 / 2;
    let frame_duration = gst::ClockTime::from_nseconds((1_000_000_000.0 / fps) as u64);

    let should_stop = Arc::new(Mutex::new(false));
    let should_stop_clone = should_stop.clone();
    let pipeline_weak = pipeline.downgrade();
    let appsrc_weak = appsrc.downgrade();

    // Запускаем поток для чтения данных из ffmpeg и передачи их в GStreamer
    let handle = thread::spawn(move || {
        let mut buffer = vec![0u8; frame_size as usize];
        let mut pts: u64 = 0;

        while !*should_stop_clone.lock().unwrap() {
            // Читаем кадр из ffmpeg
            match reader.read_exact(&mut buffer) {
                Ok(_) => {
                    // Создаем буфер GStreamer
                    if let Some(appsrc) = appsrc_weak.upgrade() {
                        let mut gst_buffer = gst::Buffer::with_size(buffer.len()).unwrap();
                        {
                            let buffer_ref = gst_buffer.make_mut();
                            buffer_ref.set_pts(gst::ClockTime::from_nseconds(pts));
                            buffer_ref.set_duration(frame_duration);

                            // Копируем данные в буфер
                            let mut map = buffer_ref.map_writable().unwrap();
                            let gst_data = map.as_mut_slice();
                            gst_data.copy_from_slice(&buffer);
                        }

                        // Отправляем буфер в пайплайн
                        let _ = appsrc.push_buffer(gst_buffer);

                        pts += frame_duration.nseconds();
                    } else {
                        break;
                    }
                }
                Err(e) => {
                    println!("Ошибка чтения из ffmpeg: {}", e);
                    break;
                }
            }
        }

        // Сигнализируем о конце потока
        if let Some(appsrc) = appsrc_weak.upgrade() {
            let _ = appsrc.end_of_stream();
        }
    });

    // Обрабатываем сообщения из шины GStreamer
    let bus = pipeline.bus().unwrap();
    for msg in bus.iter_timed(gst::ClockTime::NONE) {
        match msg.view() {
            gst::MessageView::Error(err) => {
                eprintln!(
                    "Ошибка GStreamer: {} ({:?})",
                    err.error(),
                    err.debug().unwrap_or_default()
                );
                *should_stop.lock().unwrap() = true;
                break;
            }
            gst::MessageView::Eos(_) => {
                println!("Конец потока");
                break;
            }
            _ => {}
        }
    }

    // Остановка и очистка
    *should_stop.lock().unwrap() = true;
    if let Some(pipeline) = pipeline_weak.upgrade() {
        pipeline.set_state(gst::State::Null)?;
    }

    // Ждем завершения потока чтения
    handle.join().unwrap();

    Ok(())
}
