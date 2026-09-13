use crate::geometry::Quad;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionSource {
    Contour,
    Hough,
    Fused,
}

/// Normalized detector evidence. Every field is expected to be in `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DetectionMetrics {
    pub edge_score: f32,
    pub geometry_score: f32,
    pub temporal_score: f32,
    pub agreement_score: f32,
}

impl DetectionMetrics {
    pub fn clamped(self) -> Self {
        Self {
            edge_score: self.edge_score.clamp(0.0, 1.0),
            geometry_score: self.geometry_score.clamp(0.0, 1.0),
            temporal_score: self.temporal_score.clamp(0.0, 1.0),
            agreement_score: self.agreement_score.clamp(0.0, 1.0),
        }
    }

    /// Initial weighted confidence model. Weights are deliberately centralized
    /// so they can later be calibrated against a fixture dataset.
    pub fn confidence(self) -> f32 {
        let m = self.clamped();
        0.30 * m.edge_score
            + 0.30 * m.geometry_score
            + 0.20 * m.temporal_score
            + 0.20 * m.agreement_score
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Detection {
    pub quad: Quad,
    pub source: DetectionSource,
    pub metrics: DetectionMetrics,
}

impl Detection {
    pub fn confidence(self) -> f32 {
        self.metrics.confidence()
    }
}

/// Backend-neutral detector contract.
///
/// The frame type is generic so image/imageproc, OpenCV, GPU, or test fixture
/// backends can implement the same domain contract without infecting core types.
pub trait Detector<Frame> {
    type Error;

    fn detect(&mut self, frame: &Frame) -> Result<Option<Detection>, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confidence_is_normalized() {
        let metrics = DetectionMetrics {
            edge_score: 1.2,
            geometry_score: 0.8,
            temporal_score: 0.9,
            agreement_score: -1.0,
        };
        let score = metrics.confidence();
        assert!((0.0..=1.0).contains(&score));
    }
}
