use std::io::Write;
use std::time::Duration;
use tokio::sync::broadcast;

mod audio_capture;
mod audio_processor;
mod config;
mod download;
mod real_time_transcriber;
mod silero_audio_processor;
mod stats_reporter;
mod transcribe;
mod transcription_processor;
mod transcription_stats;
mod ui;
// mod wayland_connection;

use config::read_app_config;
use download::ModelType;
use real_time_transcriber::RealTimeTranscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Loading configuration...");
    let app_config = read_app_config();
    let log_stats_enabled = app_config.log_stats_enabled;

    println!("Initializing models...");
    let (whisper_model_path, _silero_model_path) =
        download::init_all_models(Some(&app_config.model)).await?;

    println!("Whisper model ready at: {:?}", whisper_model_path);

    let mut transcriber = RealTimeTranscriber::new(whisper_model_path, app_config.clone())?;
    println!("Starting transcription automatically...");

    transcriber.start()?;

    transcriber.toggle_recording();

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel::<()>(2);
    let shutdown_tx_clone = shutdown_tx.clone();

    let transcript_history = transcriber.get_transcript_history();
    let mut transcript_rx = transcriber.get_transcript_rx();
    let audio_visualization_data = transcriber.get_audio_visualization_data();
    let audio_visualization_data_for_thread = audio_visualization_data.clone();

    tokio::spawn(async move {
        while let Ok(transcription) = transcript_rx.recv().await {
            let updated_transcript = {
                let mut history = transcript_history.write();
                if !history.is_empty() {
                    history.push(' ');
                }
                history.push_str(&transcription);
                history.clone()
            };
            let mut audio_data = audio_visualization_data_for_thread.write();
            audio_data.transcript = updated_transcript;
        }
    });

    let running = transcriber.get_running();
    let recording = transcriber.get_recording();

    ui::run_with_audio_data(audio_visualization_data, running, recording);

    Ok(())
}
