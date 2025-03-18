use anyhow;
use parking_lot::Mutex;
use portaudio as pa;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::config::read_app_config;

/// Manages audio capture using PortAudio
pub struct AudioCapture {
    pa_stream: Option<pa::Stream<pa::NonBlocking, pa::Input<f32>>>,
}

impl AudioCapture {
    /// Creates a new AudioCapture instance
    pub fn new() -> Self {
        Self { pa_stream: None }
    }

    /// Starts audio capture
    ///
    /// # Arguments
    /// * `tx` - Channel sender for audio samples
    /// * `running` - Atomic flag indicating whether the app is running
    /// * `recording` - Atomic flag indicating whether recording is active
    ///
    /// # Returns
    /// Result indicating success or error
    pub fn start(
        &mut self,
        tx: mpsc::Sender<Vec<f32>>,
        running: Arc<AtomicBool>,
        recording: Arc<AtomicBool>,
    ) -> Result<(), anyhow::Error> {
        let config = read_app_config();

        let pa = pa::PortAudio::new()
            .map_err(|e| anyhow::anyhow!("Failed to initialize PortAudio: {}", e))?;

        let input_params = pa
            .default_input_stream_params::<f32>(1)
            .map_err(|e| anyhow::anyhow!("Failed to get default input stream parameters: {}", e))?;
        let input_settings = pa::InputStreamSettings::new(
            input_params,
            config.sample_rate as f64,
            config.buffer_size as u32,
        );

        let running_clone = running.clone();
        let recording_clone = recording.clone();
        tokio::spawn(async move {
            while running_clone.load(Ordering::Relaxed) {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                if !running_clone.load(Ordering::Relaxed)
                    || !recording_clone.load(Ordering::Relaxed)
                {
                    // Signal that we should stop monitoring
                    break;
                }
            }
        });

        let callback = move |pa::InputStreamCallbackArgs { buffer, .. }| {
            if recording.load(Ordering::Relaxed) {
                let samples = buffer.to_vec();
                if let Err(e) = tx.try_send(samples) {
                    eprintln!("Failed to send samples: {}", e);
                }
            }
            if running.load(Ordering::Relaxed) {
                pa::Continue
            } else {
                pa::Complete
            }
        };

        let mut stream = pa
            .open_non_blocking_stream(input_settings, callback)
            .map_err(|e| anyhow::anyhow!("Failed to open stream: {}", e))?;

        stream
            .start()
            .map_err(|e| anyhow::anyhow!("Failed to start stream: {}", e))?;

        self.pa_stream = Some(stream);
        Ok(())
    }

    /// Temporarily pauses audio capture without closing the stream
    /// This allows for resuming the stream later
    ///
    /// # Returns
    /// Result indicating success or error
    pub fn pause(&mut self) -> Result<(), anyhow::Error> {
        if let Some(stream) = &mut self.pa_stream {
            match stream.stop() {
                Ok(_) => Ok(()),
                Err(e) => {
                    eprintln!("Failed to pause stream: {}", e);
                    Err(anyhow::anyhow!("Failed to pause stream: {}", e))
                }
            }
        } else {
            Ok(()) // No stream to pause
        }
    }

    /// Resumes a previously paused audio capture stream
    ///
    /// # Returns
    /// Result indicating success or error
    pub fn resume(&mut self) -> Result<(), anyhow::Error> {
        if let Some(stream) = &mut self.pa_stream {
            match stream.start() {
                Ok(_) => Ok(()),
                Err(e) => {
                    eprintln!("Failed to resume stream: {}", e);
                    Err(anyhow::anyhow!("Failed to resume stream: {}", e))
                }
            }
        } else {
            Err(anyhow::anyhow!("No stream to resume"))
        }
    }

    /// Completely stops and cleans up the audio capture
    /// This closes the stream and releases resources
    pub fn stop(&mut self) {
        if let Some(stream) = &mut self.pa_stream {
            if let Err(e) = stream.stop() {
                eprintln!("Failed to stop stream: {}", e);
            }
            if let Err(e) = stream.close() {
                eprintln!("Failed to close stream: {}", e);
            }
        }
        self.pa_stream = None;
    }
}

impl Drop for AudioCapture {
    fn drop(&mut self) {
        self.stop();
    }
}
