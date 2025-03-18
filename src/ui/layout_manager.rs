use super::scrollbar::SCROLLBAR_WIDTH;

pub struct LayoutManager {
    pub window_width: u32,
    pub window_height: u32,
    pub spectrogram_width: u32,
    pub spectrogram_height: u32,
    pub text_area_height: u32,
    pub right_margin: f32,
    pub left_margin: f32,
    pub gap: u32,
}

impl LayoutManager {
    pub fn new(
        window_width: u32,
        window_height: u32,
        spectrogram_width: u32,
        spectrogram_height: u32,
        text_area_height: u32,
        right_margin: f32,
        left_margin: f32,
        gap: u32,
    ) -> Self {
        Self {
            window_width,
            window_height,
            spectrogram_width,
            spectrogram_height,
            text_area_height,
            right_margin,
            left_margin,
            gap,
        }
    }

    /// Update the window dimensions
    pub fn update_dimensions(&mut self, width: u32, height: u32) {
        self.window_width = width;
        self.window_height = height;
    }

    /// Calculate the text area width, considering scrollbar if needed
    pub fn calculate_text_area_width(&self, need_scrollbar: bool) -> u32 {
        if need_scrollbar {
            self.window_width.saturating_sub(SCROLLBAR_WIDTH + 2) // Leave 2 pixels margin for visual clarity
        } else {
            self.window_width.saturating_sub(self.right_margin as u32)
        }
    }

    /// Get the effective text area height (without the gap)
    pub fn get_text_area_height(&self) -> u32 {
        self.text_area_height - self.gap
    }

    /// Get text positioning
    pub fn get_text_position(&self, scroll_offset: f32) -> (f32, f32) {
        // Fixed position for text (left margin)
        let text_x = self.left_margin;

        // Apply scroll offset to text position
        let text_y = 4.0 - scroll_offset;

        (text_x, text_y)
    }

    /// Calculate the spectrogram position
    pub fn get_spectrogram_position(&self) -> (f32, f32, f32, f32) {
        (
            0.0,                                       // x position
            (self.text_area_height + self.gap) as f32, // y position
            self.spectrogram_width as f32,             // width
            self.spectrogram_height as f32,            // height
        )
    }
}
