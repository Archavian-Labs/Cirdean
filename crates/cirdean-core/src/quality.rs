/// Quality evidence used to decide whether a stable document should be captured.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaptureQuality {
    pub motion_stability: f32,
    pub corner_stability: f32,
    pub sharpness: f32,
    pub detection_confidence: f32,
    pub exposure_stability: f32,
}

impl CaptureQuality {
    pub fn score(self) -> f32 {
        let motion = self.motion_stability.clamp(0.0, 1.0);
        let corners = self.corner_stability.clamp(0.0, 1.0);
        let sharpness = self.sharpness.clamp(0.0, 1.0);
        let detection = self.detection_confidence.clamp(0.0, 1.0);
        let exposure = self.exposure_stability.clamp(0.0, 1.0);

        0.30 * motion
            + 0.25 * corners
            + 0.20 * sharpness
            + 0.15 * detection
            + 0.10 * exposure
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QualityGate {
    pub minimum_score: f32,
    pub minimum_detection_confidence: f32,
}

impl Default for QualityGate {
    fn default() -> Self {
        Self {
            minimum_score: 0.92,
            minimum_detection_confidence: 0.85,
        }
    }
}

impl QualityGate {
    pub fn accepts(self, quality: CaptureQuality) -> bool {
        quality.score() >= self.minimum_score
            && quality.detection_confidence >= self.minimum_detection_confidence
    }
}
