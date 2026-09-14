use crate::geometry::Quad;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionSource {
    Contour,
    Hough,
    Fused,
}

/// Normalized detector evidence.
///
/// `edge_score` and `geometry_score` are mandatory because every detector can
/// produce them. Temporal and cross-detector agreement are optional evidence:
/// a strong fast-path detection must not be penalized merely because the Hough
/// fallback has not run yet or because no previous frame exists.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DetectionMetrics {
    pub edge_score: f32,
    pub geometry_score: f32,
    pub temporal_score: Option<f32>,
    pub agreement_score: Option<f32>,
}

impl DetectionMetrics {
    pub fn clamped(self) -> Self {
        Self {
            edge_score: self.edge_score.clamp(0.0, 1.0),
            geometry_score: self.geometry_score.clamp(0.0, 1.0),
            temporal_score: self.temporal_score.map(|score| score.clamp(0.0, 1.0)),
            agreement_score: self.agreement_score.map(|score| score.clamp(0.0, 1.0)),
        }
    }

    /// Confidence from the evidence that is actually available.
    ///
    /// Mandatory evidence contributes 75% of the nominal model. Optional
    /// evidence is included only when it exists and the active weights are
    /// renormalized. This keeps staged detection honest: a contour result can
    /// be accepted without first paying the cost of a Hough pass.
    pub fn confidence(self) -> f32 {
        let metrics = self.clamped();
        let mut weighted_sum = 0.40 * metrics.edge_score + 0.35 * metrics.geometry_score;
        let mut active_weight = 0.75;

        if let Some(temporal) = metrics.temporal_score {
            weighted_sum += 0.15 * temporal;
            active_weight += 0.15;
        }

        if let Some(agreement) = metrics.agreement_score {
            weighted_sum += 0.10 * agreement;
            active_weight += 0.10;
        }

        weighted_sum / active_weight
    }

    pub fn with_temporal_score(mut self, score: f32) -> Self {
        self.temporal_score = Some(score);
        self
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
            temporal_score: Some(0.9),
            agreement_score: Some(-1.0),
        };
        let score = metrics.confidence();
        assert!((0.0..=1.0).contains(&score));
    }

    #[test]
    fn missing_optional_evidence_does_not_cap_fast_detector_confidence() {
        let metrics = DetectionMetrics {
            edge_score: 1.0,
            geometry_score: 1.0,
            temporal_score: None,
            agreement_score: None,
        };

        assert!((metrics.confidence() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn optional_evidence_changes_confidence_only_when_present() {
        let base = DetectionMetrics {
            edge_score: 0.8,
            geometry_score: 0.8,
            temporal_score: None,
            agreement_score: None,
        };
        let reinforced = DetectionMetrics {
            temporal_score: Some(1.0),
            agreement_score: Some(1.0),
            ..base
        };

        assert!(reinforced.confidence() > base.confidence());
    }
}
