/// Common data structure for audio visualization
/// Used across different UI components
#[derive(Debug, Clone)]
pub struct AudioVisualizationData {
    /// Audio samples to visualize
    pub samples: Vec<f32>,
    /// Flag indicating if speech is currently detected
    pub is_speaking: bool,
    /// Current transcript text
    pub transcript: String,
    /// Flag to request resetting the transcript history
    pub reset_requested: bool,
}
