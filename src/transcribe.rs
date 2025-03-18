use ct2rs::{Whisper, WhisperOptions};
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Instant;

use crate::config;
use crate::silero_audio_processor::AudioSegment;
use crate::transcription_stats::TranscriptionStats;

/// Transcribes audio segments using Whisper model
///
/// # Arguments
/// * `whisper_arc` - Arc-wrapped Mutex containing the optional Whisper model instance
/// * `segment` - Audio segment containing samples to transcribe
/// * `language` - Language code for transcription
/// * `options` - Whisper options for controlling transcription behavior
/// * `stats` - Reference to the transcription statistics
///
/// # Returns
/// A string containing the transcription or an error message
pub fn transcribe_with_whisper(
    whisper_arc: &Arc<Mutex<Option<Whisper>>>,
    segment: &AudioSegment,
    language: &str,
    options: &WhisperOptions,
    stats: &Arc<Mutex<TranscriptionStats>>,
) -> String {
    // Get configuration options
    let app_config = config::read_app_config();
    let log_stats_enabled = app_config.log_stats_enabled;

    println!(
        "Transcribing segment from {:.2}s to {:.2}s",
        segment.start_time, segment.end_time
    );

    let start_time = Instant::now(); // Start timing
    let segment_duration = (segment.end_time - segment.start_time) as f32;

    // Get a lock on the whisper model and check if it's available
    let mut whisper_lock = whisper_arc.lock();

    if whisper_lock.is_none() {
        let total_duration = start_time.elapsed();

        if log_stats_enabled {
            println!(
                "Whisper model not available (checked in {:.2}s)",
                total_duration.as_secs_f32()
            );
        }

        return "[whisper model not available]".to_string();
    }

    // Generate with the model while still holding the lock
    let whisper = whisper_lock.as_ref().unwrap();
    let inference_start = Instant::now();

    let result = match whisper.generate(&segment.samples, Some(language), false, options) {
        Ok(result) => {
            let inference_duration = inference_start.elapsed();
            let total_duration = start_time.elapsed();
            let inference_secs = inference_duration.as_secs_f32();
            let total_secs = total_duration.as_secs_f32();

            // Update statistics
            if let Some(mut stats_lock) = stats.try_lock() {
                stats_lock.update(segment_duration, inference_secs, total_secs);
            }

            let transcription = result
                .first()
                .map_or("[no transcription]".to_string(), |s| s.to_string());

            if log_stats_enabled {
                println!(
                    "Transcription timing: Segment length: {:.2}s, Inference time: {:.2}s, Total processing time: {:.2}s, RTF: {:.2}",
                    segment_duration,
                    inference_secs,
                    total_secs,
                    inference_secs / segment_duration
                );

                println!("Transcription: '{}'", transcription);
            }

            transcription
        }
        Err(e) => {
            let total_duration = start_time.elapsed();

            // Only log if enabled
            if log_stats_enabled {
                println!(
                    "Transcription error after {:.2}s: {}",
                    total_duration.as_secs_f32(),
                    e
                );
            }

            format!("[transcription error: {}]", e)
        }
    };

    drop(whisper_lock);

    result
}
