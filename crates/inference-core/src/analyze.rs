use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViolationConfig {
    pub max_people_allowed: usize,
    pub require_face_recognition: bool,
    pub min_recognition_percentage: f32, // 0.0 - 1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ViolationType {
    ExcessivePeople {
        detected: usize,
        max_allowed: usize,
    },
    UnrecognizedFaces {
        total_people: usize,
        recognized_faces: usize,
        recognition_percentage: f32,
        required_percentage: f32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViolationReport {
    pub violations: Vec<ViolationType>,
    pub has_violations: bool,
    pub total_frames_processed: usize,
    pub max_people_detected: usize,
    pub recognition_stats: RecognitionStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecognitionStats {
    pub total_face_detections: usize,
    pub recognized_faces: usize,
    pub unique_recognized_people: HashMap<String, usize>,
    pub recognition_rate: f32,
}

#[derive(Debug, Clone)]
pub struct FrameAnalysis {
    pub people_count: usize,
    pub faces_detected: usize,
    pub recognized_faces: Vec<String>,
}

pub struct VideoViolationDetector {
    config: ViolationConfig,
    frame_analyses: Vec<FrameAnalysis>,
    max_people_detected: usize,
    total_face_detections: usize,
    total_recognized_faces: usize,
    recognized_people_stats: HashMap<String, usize>,
}

impl VideoViolationDetector {
    pub fn new(config: ViolationConfig) -> Self {
        Self {
            config,
            frame_analyses: Vec::new(),
            max_people_detected: 0,
            total_face_detections: 0,
            total_recognized_faces: 0,
            recognized_people_stats: HashMap::new(),
        }
    }

    pub fn add_frame_analysis(&mut self, analysis: FrameAnalysis) {
        if analysis.people_count > self.max_people_detected {
            self.max_people_detected = analysis.people_count;
        }

        self.total_face_detections += analysis.faces_detected;
        self.total_recognized_faces += analysis.recognized_faces.len();

        for person_name in &analysis.recognized_faces {
            *self
                .recognized_people_stats
                .entry(person_name.clone())
                .or_insert(0) += 1;
        }

        self.frame_analyses.push(analysis);
    }

    pub fn generate_violation_report(&self) -> ViolationReport {
        let mut violations = Vec::new();

        if self.max_people_detected > self.config.max_people_allowed {
            violations.push(ViolationType::ExcessivePeople {
                detected: self.max_people_detected,
                max_allowed: self.config.max_people_allowed,
            });
        }

        if self.config.require_face_recognition && self.total_face_detections > 0 {
            let recognition_rate =
                self.total_recognized_faces as f32 / self.total_face_detections as f32;

            if recognition_rate < self.config.min_recognition_percentage {
                violations.push(ViolationType::UnrecognizedFaces {
                    total_people: self.max_people_detected,
                    recognized_faces: self.total_recognized_faces,
                    recognition_percentage: recognition_rate,
                    required_percentage: self.config.min_recognition_percentage,
                });
            }
        }

        let recognition_stats = RecognitionStats {
            total_face_detections: self.total_face_detections,
            recognized_faces: self.total_recognized_faces,
            unique_recognized_people: self.recognized_people_stats.clone(),
            recognition_rate: if self.total_face_detections > 0 {
                self.total_recognized_faces as f32 / self.total_face_detections as f32
            } else {
                0.0
            },
        };

        ViolationReport {
            has_violations: !violations.is_empty(),
            violations,
            total_frames_processed: self.frame_analyses.len(),
            max_people_detected: self.max_people_detected,
            recognition_stats,
        }
    }

    pub fn reset(&mut self) {
        self.frame_analyses.clear();
        self.max_people_detected = 0;
        self.total_face_detections = 0;
        self.total_recognized_faces = 0;
        self.recognized_people_stats.clear();
    }

    pub fn finalize_and_print_report(&self) {
        let report = self.generate_violation_report();
        report.print_report();

        if report.has_violations {
            self.send_violation_alert(&report);
        }
    }

    fn send_violation_alert(&self, report: &ViolationReport) {
        println!("🚨 ATTENTION: Violations detected in the video!");
        // database::save_violation_report(report);
    }
}

impl ViolationReport {
    pub fn print_report(&self) {
        println!("=== ОТЧЕТ О НАРУШЕНИЯХ ===");
        println!("Общих кадров обработано: {}", self.total_frames_processed);
        println!(
            "Максимальное количество людей: {}",
            self.max_people_detected
        );
        println!("Статистика распознавания:");
        println!(
            "  - Всего лиц обнаружено: {}",
            self.recognition_stats.total_face_detections
        );
        println!(
            "  - Распознано лиц: {}",
            self.recognition_stats.recognized_faces
        );
        println!(
            "  - Процент распознавания: {:.1}%",
            self.recognition_stats.recognition_rate * 100.0
        );

        if !self.recognition_stats.unique_recognized_people.is_empty() {
            println!("  - Уникальные люди:");
            for (name, count) in &self.recognition_stats.unique_recognized_people {
                println!("    * {}: {} кадров", name, count);
            }
        }

        if self.has_violations {
            println!("\n🚨 ОБНАРУЖЕНЫ НАРУШЕНИЯ:");
            for (i, violation) in self.violations.iter().enumerate() {
                match violation {
                    ViolationType::ExcessivePeople {
                        detected,
                        max_allowed,
                    } => {
                        println!("  {}. Превышено максимальное количество людей:", i + 1);
                        println!("     Обнаружено: {}, Допустимо: {}", detected, max_allowed);
                    }
                    ViolationType::UnrecognizedFaces {
                        total_people,
                        recognized_faces,
                        recognition_percentage,
                        required_percentage,
                    } => {
                        println!("  {}. Недостаточное распознавание лиц:", i + 1);
                        println!("     Всего людей: {}", total_people);
                        println!("     Распознано лиц: {}", recognized_faces);
                        println!(
                            "     Процент распознавания: {:.1}%",
                            recognition_percentage * 100.0
                        );
                        println!(
                            "     Требуется минимум: {:.1}%",
                            required_percentage * 100.0
                        );
                    }
                }
            }
        } else {
            println!("\n✅ НАРУШЕНИЙ НЕ ОБНАРУЖЕНО");
        }
        println!("========================");
    }
}
