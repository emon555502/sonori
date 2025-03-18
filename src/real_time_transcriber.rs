use anyhow::Context;
use ct2rs::{ComputeType, Config, Device, Whisper, WhisperOptions};
use parking_lot::{Mutex, RwLock};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};

// Use local modules
use crate::audio_capture::AudioCapture;
use crate::audio_processor::AudioProcessor;
use crate::config::{read_app_config, AppConfig};
use crate::silero_audio_processor::{AudioSegment, SileroVad};
use crate::stats_reporter::StatsReporter;
use crate::transcription_processor::TranscriptionProcessor;
use crate::transcription_stats::TranscriptionStats;
use crate::ui::common::AudioVisualizationData;

/// Main transcription coordinator that integrates all components
pub struct RealTimeTranscriber {
    // Audio capture
    audio_capture: AudioCapture,

    // Audio processing
    tx: mpsc::Sender<Vec<f32>>,
    rx: Option<mpsc::Receiver<Vec<f32>>>,

    // Transcription
    pub transcript_tx: broadcast::Sender<String>,
    pub transcript_rx: broadcast::Receiver<String>,

    // State control
    running: Arc<AtomicBool>,
    recording: Arc<AtomicBool>,

    // Model and parameters
    whisper: Arc<Mutex<Option<Whisper>>>,
    language: String,
    options: WhisperOptions,

    // Processing components
    audio_processor: Arc<Mutex<SileroVad>>,

    // Data storage and visualization
    transcript_history: Arc<RwLock<String>>,
    audio_visualization_data: Arc<RwLock<AudioVisualizationData>>,

    // Communication channels for sub-components
    segment_tx: mpsc::Sender<AudioSegment>,
    segment_rx: Option<mpsc::Receiver<AudioSegment>>,
    transcription_done_tx: mpsc::UnboundedSender<()>,
    transcription_done_rx: Option<mpsc::UnboundedReceiver<()>>,

    // Statistics
    transcription_stats: Arc<Mutex<TranscriptionStats>>,
    stats_reporter: Option<StatsReporter>,

    // Sub-components
    transcription_processor: Option<TranscriptionProcessor>,
    audio_processor_component: Option<AudioProcessor>,
}

impl RealTimeTranscriber {
    /// Creates a new RealTimeTranscriber instance
    ///
    /// # Arguments
    /// * `model_path` - Path to the Whisper model file
    /// * `app_config` - Application configuration
    ///
    /// # Returns
    /// Result containing the new instance or an error
    pub fn new(model_path: PathBuf, app_config: AppConfig) -> Result<Self, anyhow::Error> {
        // Use bounded channels with appropriate capacities for better backpressure
        // 10 is a good default capacity for audio data that ensures we don't queue too much
        let (tx, rx) = mpsc::channel(10);
        let (transcript_tx, transcript_rx) = broadcast::channel(100);
        // Use a bounded channel for audio segments as well
        let (segment_tx, segment_rx) = mpsc::channel(10);
        // Keep this one unbounded since it's just for signaling completion
        let (transcription_done_tx, transcription_done_rx) = mpsc::unbounded_channel();

        // Get the Silero model from the models directory
        let home_dir = std::env::var("HOME").with_context(|| "Failed to get HOME directory")?;
        let models_dir = PathBuf::from(format!("{}/.cache/sonori/models", home_dir));
        let silero_model_path = models_dir.join("silero_vad.onnx");

        if !silero_model_path.exists() {
            return Err(anyhow::anyhow!(
                "Silero VAD model not found at {}. Please run the application again to download it.",
                silero_model_path.display()
            ));
        }

        println!("Using Silero VAD model at: {:?}", silero_model_path);
        println!("Using Whisper model at: {:?}", model_path);

        let running = Arc::new(AtomicBool::new(true));
        let recording = Arc::new(AtomicBool::new(false));
        let transcript_history = Arc::new(RwLock::new(String::new()));
        let whisper = Arc::new(Mutex::new(None));
        let transcription_stats = Arc::new(Mutex::new(TranscriptionStats::new()));

        let audio_visualization_data = Arc::new(RwLock::new(AudioVisualizationData {
            samples: Vec::new(),
            is_speaking: false,
            transcript: String::new(),
            reset_requested: false,
        }));

        let compute_type = match app_config.compute_type.as_str() {
            "FLOAT16" => ComputeType::FLOAT16,
            "INT8" => ComputeType::INT8,
            _ => ComputeType::INT8,
        };

        let audio_processor = match SileroVad::new(
            (
                app_config.vad_config.clone(),
                app_config.buffer_size,
                app_config.sample_rate,
            )
                .into(),
            &silero_model_path,
        ) {
            Ok(vad) => Arc::new(Mutex::new(vad)),
            Err(e) => {
                eprintln!(
                    "Failed to initialize SileroVad: {}. Using default configuration might help.",
                    e
                );
                return Err(anyhow::anyhow!("VAD initialization failed: {}", e));
            }
        };

        let whisper_clone = whisper.clone();
        let model_path_clone = model_path.clone();
        let options = app_config.whisper_options.to_whisper_options();

        tokio::spawn(async move {
            let mut config = Config::default();
            config.device = Device::CPU;
            config.compute_type = compute_type;
            config.num_threads_per_replica = 8;

            match Whisper::new(&model_path_clone, config) {
                Ok(w) => {
                    println!("Whisper model loaded successfully!");
                    *whisper_clone.lock() = Some(w);
                }
                Err(e) => {
                    eprintln!("Failed to load Whisper model: {}", e);
                }
            }
        });

        Ok(Self {
            audio_capture: AudioCapture::new(),
            tx,
            rx: Some(rx),
            transcript_tx,
            transcript_rx,
            running,
            recording,
            whisper,
            language: app_config.language,
            options,
            audio_processor,
            transcript_history,
            audio_visualization_data,
            segment_tx,
            segment_rx: Some(segment_rx),
            transcription_done_tx,
            transcription_done_rx: Some(transcription_done_rx),
            transcription_stats,
            stats_reporter: None,
            transcription_processor: None,
            audio_processor_component: None,
        })
    }

    /// Starts the audio capture and transcription process
    ///
    /// Sets up PortAudio for capturing audio and spawns worker tasks for processing
    ///
    /// # Returns
    /// Result indicating success or an error with detailed message
    pub fn start(&mut self) -> Result<(), anyhow::Error> {
        // Ensure recording is initially set to false
        self.recording.store(false, Ordering::Relaxed);

        // Set running to true
        self.running.store(true, Ordering::Relaxed);

        // Start audio capture
        self.audio_capture.start(
            self.tx.clone(),
            self.running.clone(),
            self.recording.clone(),
        )?;

        // Initialize statistics reporter
        let stats_reporter =
            StatsReporter::new(self.transcription_stats.clone(), self.running.clone());
        stats_reporter.start_periodic_reporting();
        self.stats_reporter = Some(stats_reporter);

        // Initialize transcription processor
        let transcription_processor = TranscriptionProcessor::new(
            self.whisper.clone(),
            self.language.clone(),
            self.options.clone(),
            self.running.clone(),
            self.transcription_done_tx.clone(),
            self.transcription_stats.clone(),
        );

        // Store the processor first
        self.transcription_processor = Some(transcription_processor);

        // Get config
        let config = read_app_config();

        // Initialize audio processor
        let audio_processor = AudioProcessor::new(
            self.running.clone(),
            self.recording.clone(),
            self.transcript_history.clone(),
            self.audio_processor.clone(),
            self.audio_visualization_data.clone(),
            self.segment_tx.clone(),
            config,
        );

        // Store the processor first
        self.audio_processor_component = Some(audio_processor);

        // Take ownership of the receivers and pass them to the processors
        if let (Some(processor_a), Some(segment_rx)) =
            (&self.audio_processor_component, self.segment_rx.take())
        {
            if let (Some(processor_t), Some(rx)) = (&self.transcription_processor, self.rx.take()) {
                processor_t.start(segment_rx, self.transcript_tx.clone());
                processor_a.start(rx);
            }
        }

        Ok(())
    }

    /// Stops the audio capture and transcription process
    ///
    /// Terminates all audio processing and releases resources
    ///
    /// # Returns
    /// Result indicating success or an error with detailed message
    pub async fn stop(&mut self) -> Result<(), anyhow::Error> {
        self.recording.store(false, Ordering::Relaxed);

        // We don't set running to false because we want to be able to resume

        // Pause the audio stream without closing it
        if let Err(e) = self.audio_capture.pause() {
            eprintln!("Warning: Failed to pause audio capture: {}", e);
        }

        Ok(())
    }

    /// Resumes audio processing after it has been stopped
    ///
    /// # Returns
    /// Result indicating success or error
    pub async fn resume(&mut self) -> Result<(), anyhow::Error> {
        // Resume the audio stream
        if let Err(e) = self.audio_capture.resume() {
            return Err(anyhow::anyhow!("Failed to resume audio capture: {}", e));
        }

        // Set recording back to true if it was previously recording
        self.recording.store(true, Ordering::Relaxed);

        Ok(())
    }

    /// Completely shuts down the audio capture and transcription process
    ///
    /// Terminates all audio processing and releases resources
    ///
    /// # Returns
    /// Result indicating success or an error with detailed message
    pub async fn shutdown(&mut self) -> Result<(), anyhow::Error> {
        self.running.store(false, Ordering::Relaxed);
        self.recording.store(false, Ordering::Relaxed);

        // Create a timeout for waiting on the transcription thread
        if let Some(rx) = &mut self.transcription_done_rx {
            match tokio::time::timeout(Duration::from_secs(5), rx.recv()).await {
                Ok(_) => (),
                Err(_) => eprintln!("Timeout waiting for transcription thread to finish"),
            }
        }

        // Completely stop and clean up the audio capture
        self.audio_capture.stop();

        Ok(())
    }

    /// Toggles the recording state between active and inactive
    ///
    /// When active, audio is captured and processed for transcription
    pub fn toggle_recording(&mut self) {
        let was_recording = self.recording.load(Ordering::Relaxed);
        self.recording.store(!was_recording, Ordering::Relaxed);

        // Toggle the audio stream based on the new recording state
        if was_recording {
            // We were recording, now stopping - pause the stream
            if let Err(e) = self.audio_capture.pause() {
                // It's okay if the stream is not started - that's an expected edge case
                if !e.to_string().contains("StreamIsNotStarted") {
                    eprintln!("Warning: Failed to pause audio stream: {}", e);
                }
            }
        } else {
            // We were stopped, now recording - resume the stream
            if let Err(e) = self.audio_capture.resume() {
                // It's okay if the stream is not stopped - that's an expected edge case
                if !e.to_string().contains("StreamIsNotStopped") {
                    eprintln!("Warning: Failed to resume audio stream: {}", e);
                }
            }
        }

        println!("Recording toggled to: {}", !was_recording);
    }

    /// Returns the current transcript history
    ///
    /// # Returns
    /// A string containing all transcribed text so far
    pub fn get_transcript(&self) -> String {
        match self.transcript_history.try_read() {
            Some(history) => history.clone(),
            None => self.transcript_history.read().clone(),
        }
    }

    /// Returns the transcription statistics
    ///
    /// # Returns
    /// A formatted string containing transcription performance statistics
    pub fn get_stats_report(&self) -> String {
        match self.transcription_stats.try_lock() {
            Some(stats) => stats.report(),
            None => self.transcription_stats.lock().report(),
        }
    }

    /// Prints the current transcription statistics to console
    ///
    /// Useful for debugging or on-demand performance reporting
    pub fn print_stats(&self) {
        if let Some(stats_reporter) = &self.stats_reporter {
            stats_reporter.print_stats();
        }
    }

    /// Get the audio visualization data reference
    pub fn get_audio_visualization_data(&self) -> Arc<RwLock<AudioVisualizationData>> {
        self.audio_visualization_data.clone()
    }

    /// Get the running state reference
    pub fn get_running(&self) -> Arc<AtomicBool> {
        self.running.clone()
    }

    /// Get the recording state reference
    pub fn get_recording(&self) -> Arc<AtomicBool> {
        self.recording.clone()
    }

    /// Get the transcript history reference
    pub fn get_transcript_history(&self) -> Arc<RwLock<String>> {
        self.transcript_history.clone()
    }

    /// Get the transcript receiver for listening to new transcriptions
    pub fn get_transcript_rx(&self) -> broadcast::Receiver<String> {
        self.transcript_tx.subscribe()
    }
}

impl Drop for RealTimeTranscriber {
    fn drop(&mut self) {
        // We need to manually do the cleanup since we can't use the async shutdown method
        self.running.store(false, Ordering::Relaxed);
        self.recording.store(false, Ordering::Relaxed);

        // Wait for transcription to finish with a timeout
        if let Some(mut rx) = self.transcription_done_rx.take() {
            let start = std::time::Instant::now();
            let timeout = Duration::from_secs(1);

            while start.elapsed() < timeout {
                match rx.try_recv() {
                    Ok(_) => break, // Received shutdown confirmation
                    Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                        std::thread::sleep(Duration::from_millis(50));
                        continue;
                    }
                    Err(_) => break, // Channel closed or other error
                }
            }

            if start.elapsed() >= timeout {
                eprintln!("Timeout waiting for transcription thread to finish during cleanup");
            }
        }

        // Completely stop the audio capture
        self.audio_capture.stop();
        *self.whisper.lock() = None;
        println!("Cleaned up RealTimeTranscriber resources");
    }
}
