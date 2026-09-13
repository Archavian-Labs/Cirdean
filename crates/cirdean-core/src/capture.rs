use crate::{CaptureQuality, Detection, Quad, QualityGate};

/// Auto-capture policy derived from Hbot's stable-frame/re-arm workflow, but
/// expressed in backend-neutral evidence so Cirdean can use document-local
/// motion, corner motion, or future optical-flow signals.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AutoCaptureConfig {
    pub stable_for_ms: u64,
    pub maximum_stable_corner_shift: f32,
    pub rearm_corner_shift: f32,
    pub minimum_page_change_score: f32,
    pub quality_gate: QualityGate,
}

impl Default for AutoCaptureConfig {
    fn default() -> Self {
        Self {
            stable_for_ms: 650,
            maximum_stable_corner_shift: 8.0,
            rearm_corner_shift: 24.0,
            minimum_page_change_score: 0.35,
            quality_gate: QualityGate::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CaptureState {
    Searching,
    Stabilizing { started_at_ms: u64, anchor: Quad },
    AwaitingPageChange { captured: Quad },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CaptureEvent {
    Capture(Detection),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AutoCaptureController {
    config: AutoCaptureConfig,
    state: CaptureState,
}

impl AutoCaptureController {
    pub const fn new(config: AutoCaptureConfig) -> Self {
        Self {
            config,
            state: CaptureState::Searching,
        }
    }

    pub const fn state(self) -> CaptureState {
        self.state
    }

    pub fn reset(&mut self) {
        self.state = CaptureState::Searching;
    }

    /// Consume one preview observation.
    ///
    /// `scene_change_score` is intentionally separate from corner motion. A
    /// page can be replaced while occupying almost exactly the same rectangle,
    /// which is why Hbot's page-turn motion signal is useful. Backends should
    /// preferably compute this score inside the detected document region rather
    /// than across the entire camera frame.
    pub fn update(
        &mut self,
        now_ms: u64,
        detection: Option<Detection>,
        mut quality: CaptureQuality,
        scene_change_score: f32,
    ) -> Option<CaptureEvent> {
        let scene_changed = scene_change_score.clamp(0.0, 1.0)
            >= self.config.minimum_page_change_score;

        match self.state {
            CaptureState::Searching => {
                if let Some(detection) = detection {
                    self.state = CaptureState::Stabilizing {
                        started_at_ms: now_ms,
                        anchor: detection.quad,
                    };
                }
                None
            }
            CaptureState::Stabilizing {
                started_at_ms,
                anchor,
            } => {
                let Some(detection) = detection else {
                    self.state = CaptureState::Searching;
                    return None;
                };

                if anchor.average_corner_distance(detection.quad)
                    > self.config.maximum_stable_corner_shift
                    || scene_changed
                {
                    self.state = CaptureState::Stabilizing {
                        started_at_ms: now_ms,
                        anchor: detection.quad,
                    };
                    return None;
                }

                quality.detection_confidence = detection.confidence();
                let stable_long_enough =
                    now_ms.saturating_sub(started_at_ms) >= self.config.stable_for_ms;

                if stable_long_enough && self.config.quality_gate.accepts(quality) {
                    self.state = CaptureState::AwaitingPageChange {
                        captured: detection.quad,
                    };
                    Some(CaptureEvent::Capture(detection))
                } else {
                    None
                }
            }
            CaptureState::AwaitingPageChange { captured } => {
                let moved_to_new_document = detection.is_some_and(|current| {
                    captured.average_corner_distance(current.quad) > self.config.rearm_corner_shift
                });

                if scene_changed || moved_to_new_document {
                    self.state = if let Some(current) = detection {
                        CaptureState::Stabilizing {
                            started_at_ms: now_ms,
                            anchor: current.quad,
                        }
                    } else {
                        CaptureState::Searching
                    };
                }

                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DetectionMetrics, DetectionSource, Point};

    fn detection(offset: f32) -> Detection {
        Detection {
            quad: Quad::new(
                Point::new(offset, 0.0),
                Point::new(100.0 + offset, 0.0),
                Point::new(100.0 + offset, 200.0),
                Point::new(offset, 200.0),
            ),
            source: DetectionSource::Contour,
            metrics: DetectionMetrics {
                edge_score: 1.0,
                geometry_score: 1.0,
                temporal_score: None,
                agreement_score: None,
            },
        }
    }

    fn excellent_quality() -> CaptureQuality {
        CaptureQuality {
            motion_stability: 1.0,
            corner_stability: 1.0,
            sharpness: 1.0,
            detection_confidence: 0.0,
            exposure_stability: 1.0,
        }
    }

    #[test]
    fn stable_document_is_captured_only_once() {
        let mut controller = AutoCaptureController::new(AutoCaptureConfig::default());
        assert_eq!(
            controller.update(0, Some(detection(0.0)), excellent_quality(), 0.0),
            None
        );
        assert!(matches!(
            controller.update(700, Some(detection(1.0)), excellent_quality(), 0.0),
            Some(CaptureEvent::Capture(_))
        ));
        assert_eq!(
            controller.update(1_500, Some(detection(1.0)), excellent_quality(), 0.0),
            None
        );
    }

    #[test]
    fn scene_change_rearms_even_when_new_page_has_same_geometry() {
        let mut controller = AutoCaptureController::new(AutoCaptureConfig::default());
        controller.update(0, Some(detection(0.0)), excellent_quality(), 0.0);
        assert!(controller
            .update(700, Some(detection(0.0)), excellent_quality(), 0.0)
            .is_some());

        assert_eq!(
            controller.update(800, Some(detection(0.0)), excellent_quality(), 0.8),
            None
        );
        assert!(matches!(
            controller.state(),
            CaptureState::Stabilizing { .. }
        ));
        assert!(controller
            .update(1_500, Some(detection(0.0)), excellent_quality(), 0.0)
            .is_some());
    }
}
