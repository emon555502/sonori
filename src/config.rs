// use crate::audio_processor::VadConfig;
use crate::silero_audio_processor::VadConfig as SileroVadConfig;
use ct2rs::WhisperOptions;
use serde::{Deserialize, Serialize};

/// Audio processor configuration parameters for general audio processing
/// This is separate from the VAD-specific settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioProcessorConfig {
    /// Maximum number of samples to store for visualization
    /// Controls the detail level of the audio waveform display
    pub max_vis_samples: usize,
}

impl Default for AudioProcessorConfig {
    fn default() -> Self {
        Self {
            max_vis_samples: 1024, // Number of samples to display in visualization
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Main model to use for transcription
    pub model: String,
    /// Language for transcription
    pub language: String,
    /// Compute type for model inference
    pub compute_type: String,
    /// Whether to log statistics
    pub log_stats_enabled: bool,
    /// The global buffer size used throughout the application
    /// This is the fundamental audio processing block size in samples
    pub buffer_size: usize,
    /// Audio sample rate in Hz (must be 8000 or 16000 for Silero VAD)
    /// This value is used throughout the application for audio processing
    pub sample_rate: usize,
    /// Whisper model configuration
    pub whisper_options: WhisperOptionsSerde,
    /// Voice Activity Detection configuration
    pub vad_config: VadConfigSerde,
    /// Audio processor configuration
    pub audio_processor_config: AudioProcessorConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhisperOptionsSerde {
    pub beam_size: usize,
    pub patience: f32,
    pub repetition_penalty: f32,
}

/// Configuration for Voice Activity Detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VadConfigSerde {
    /// Probability threshold for speech detection (0.0-1.0)
    pub threshold: f32,
    /// Number of frames before confirming speech
    pub hangbefore_frames: usize,
    /// Number of frames after speech before ending segment
    pub hangover_frames: usize,
    /// Maximum buffer size in seconds
    pub max_buffer_duration_sec: f32,
    /// Maximum number of segments to keep
    pub max_segment_count: usize,
}

impl Default for VadConfigSerde {
    fn default() -> Self {
        Self {
            threshold: 0.2,                // Silero uses probability threshold (0.0-1.0)
            hangbefore_frames: 1,          // Wait for this many frames before confirming speech
            hangover_frames: 15, // Wait for this many frames of silence before ending segment
            max_buffer_duration_sec: 30.0, // Maximum buffer size in seconds
            max_segment_count: 20, // Maximum number of segments to keep
        }
    }
}

impl SileroVadConfig {
    pub fn from_config(
        vad_config: &VadConfigSerde,
        buffer_size: usize,
        sample_rate: usize,
    ) -> Self {
        Self {
            threshold: vad_config.threshold,
            frame_size: buffer_size,
            sample_rate,
            hangbefore_frames: vad_config.hangbefore_frames,
            hangover_frames: vad_config.hangover_frames,
            max_buffer_duration: (vad_config.max_buffer_duration_sec * sample_rate as f32) as usize,
            max_segment_count: vad_config.max_segment_count,
        }
    }
}

impl From<(VadConfigSerde, usize, usize)> for SileroVadConfig {
    fn from((config, buffer_size, sample_rate): (VadConfigSerde, usize, usize)) -> Self {
        Self {
            threshold: config.threshold,
            frame_size: buffer_size,
            sample_rate,
            hangbefore_frames: config.hangbefore_frames,
            hangover_frames: config.hangover_frames,
            max_buffer_duration: (config.max_buffer_duration_sec * sample_rate as f32) as usize,
            max_segment_count: config.max_segment_count,
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            model: "openai/whisper-base.en".to_string(),
            language: "en".to_string(),
            compute_type: "INT8".to_string(),
            log_stats_enabled: true,
            buffer_size: 1024,
            sample_rate: 16000, // 16kHz (supported by Silero VAD)
            whisper_options: WhisperOptionsSerde {
                beam_size: 5,
                patience: 1.0,
                repetition_penalty: 1.25,
            },
            vad_config: VadConfigSerde::default(),
            audio_processor_config: AudioProcessorConfig::default(),
        }
    }
}

impl WhisperOptionsSerde {
    pub fn to_whisper_options(&self) -> WhisperOptions {
        WhisperOptions {
            beam_size: self.beam_size,
            patience: self.patience,
            repetition_penalty: self.repetition_penalty,
            ..Default::default()
        }
    }
}

/// Helper function to read the application configuration
pub fn read_app_config() -> AppConfig {
    match std::fs::read_to_string("config.json") {
        Ok(config_str) => match serde_json::from_str(&config_str) {
            Ok(config) => config,
            Err(e) => {
                println!(
                    "Failed to parse config.json: {}. Using default configuration.",
                    e
                );
                AppConfig::default()
            }
        },
        Err(e) => {
            println!(
                "Failed to read config.json: {}. Using default configuration.",
                e
            );
            AppConfig::default()
        }
    }
}
