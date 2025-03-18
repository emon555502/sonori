use parking_lot::{Mutex, RwLock};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::Duration;

use crate::config::{AppConfig, AudioProcessorConfig};
use crate::silero_audio_processor::{AudioSegment, SileroVad, VadState};
use crate::ui::common::AudioVisualizationData;

/// Handles audio processing and voice activity detection
pub struct AudioProcessor {
    running: Arc<Mutex<bool>>,
    recording: Arc<Mutex<bool>>,
    transcript_history: Arc<RwLock<String>>,
    audio_processor: Arc<Mutex<SileroVad>>,
    audio_visualization_data: Arc<RwLock<AudioVisualizationData>>,
    segment_tx: mpsc::Sender<AudioSegment>,
    buffer_size: usize,
    config: AudioProcessorConfig,
}

impl AudioProcessor {
    pub fn new(
        running: Arc<Mutex<bool>>,
        recording: Arc<Mutex<bool>>,
        transcript_history: Arc<RwLock<String>>,
        audio_processor: Arc<Mutex<SileroVad>>,
        audio_visualization_data: Arc<RwLock<AudioVisualizationData>>,
        segment_tx: mpsc::Sender<AudioSegment>,
        app_config: AppConfig,
    ) -> Self {
        Self {
            running,
            recording,
            transcript_history,
            audio_processor,
            audio_visualization_data,
            segment_tx,
            buffer_size: app_config.buffer_size,
            config: app_config.audio_processor_config,
        }
    }

    /// Starts audio processing
    pub fn start(&self, mut rx: mpsc::Receiver<Vec<f32>>) {
        let running = self.running.clone();
        let recording = self.recording.clone();
        let transcript_history = self.transcript_history.clone();
        let audio_processor = self.audio_processor.clone();
        let audio_visualization_data = self.audio_visualization_data.clone();
        let segment_tx = self.segment_tx.clone();
        let config = self.config.clone();
        let buffer_size = self.buffer_size;

        // Create thread-local buffer
        let mut audio_buffer = VecDeque::with_capacity(buffer_size);
        let max_vis_samples = config.max_vis_samples;

        // Start audio processing task
        tokio::spawn(async move {
            let mut _last_vad_state = VadState::Silence;
            let mut latest_is_speaking = false;

            while *running.lock() {
                if !*recording.lock() {
                    tokio::time::sleep(Duration::from_millis(16)).await;
                    continue;
                }

                match rx.try_recv() {
                    Ok(samples) => {
                        // Clear buffer and add new samples
                        audio_buffer.clear();
                        audio_buffer.extend(samples.iter().cloned());

                        // Use non-blocking try_lock for UI data to prevent blocking
                        if let Some(mut audio_data) = audio_visualization_data.try_write() {
                            // Create visualization samples from the audio buffer
                            audio_data.samples.clear();
                            audio_data
                                .samples
                                .extend(audio_buffer.iter().take(max_vis_samples).cloned());
                        }

                        let segments = if let Some(mut processor) = audio_processor.try_lock() {
                            match processor
                                .process_audio(&Vec::from_iter(audio_buffer.iter().cloned()))
                            {
                                Ok(segments) => {
                                    latest_is_speaking = processor.is_speaking();

                                    // Use non-blocking try_lock for UI updates
                                    let reset_requested = {
                                        if let Some(mut audio_data) =
                                            audio_visualization_data.try_write()
                                        {
                                            audio_data.is_speaking = latest_is_speaking;
                                            let reset = audio_data.reset_requested;
                                            if reset {
                                                audio_data.reset_requested = false;
                                                audio_data.transcript.clear();
                                            }
                                            reset
                                        } else {
                                            false
                                        }
                                    };

                                    if reset_requested {
                                        if let Some(mut history) = transcript_history.try_write() {
                                            history.clear();
                                        }
                                    }

                                    _last_vad_state = processor.get_state();
                                    segments
                                }
                                Err(e) => {
                                    eprintln!("Error processing audio: {}", e);
                                    Vec::new()
                                }
                            }
                        } else {
                            // Could not get lock on processor, skip this batch
                            Vec::new()
                        };

                        // Send segments for transcription
                        for segment in segments {
                            if let Err(e) = segment_tx.try_send(segment) {
                                eprintln!("Failed to send audio segment: {}", e);
                            }
                        }
                    }
                    Err(mpsc::error::TryRecvError::Empty) => {
                        // Try to update visualization even if no new samples
                        if !audio_buffer.is_empty() {
                            if let Some(mut audio_data) = audio_visualization_data.try_write() {
                                if audio_data.samples.is_empty() {
                                    audio_data.samples.clear();
                                    audio_data
                                        .samples
                                        .extend(audio_buffer.iter().take(max_vis_samples).cloned());
                                    audio_data.is_speaking = latest_is_speaking;
                                }
                            }
                        }
                        tokio::time::sleep(Duration::from_millis(16)).await;
                    }
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        println!("Audio channel disconnected");
                        break;
                    }
                }
            }
        });
    }
}
