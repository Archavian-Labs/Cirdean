use std::convert::Infallible;

use cirdean_core::{Detection, DetectionSource, Detector};
use image::GrayImage;

use crate::{
    candidates::{corner_distance, rank_distinct},
    contour::ContourDetector,
    hough::HoughDetector,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HybridConfig {
    /// Skip Hough only when the best eligible contour is also unambiguous.
    pub fast_accept_confidence: f32,
    /// Agreement distance normalized to the longest source-frame dimension.
    pub fusion_corner_distance_ratio: f32,
    pub minimum_fusion_agreement: f32,
    /// Abstain when geometrically distinct hypotheses have near-equal scores.
    pub minimum_candidate_margin: f32,
    pub minimum_confidence: f32,
    pub minimum_edge_support: f32,
}

impl Default for HybridConfig {
    fn default() -> Self {
        Self {
            fast_accept_confidence: 0.88,
            fusion_corner_distance_ratio: 0.04,
            minimum_fusion_agreement: 0.50,
            minimum_candidate_margin: 0.04,
            minimum_confidence: 0.65,
            minimum_edge_support: 0.55,
        }
    }
}

#[derive(Debug, Default)]
pub struct HybridDetector {
    contour: ContourDetector,
    hough: HoughDetector,
    config: HybridConfig,
}

impl HybridDetector {
    pub const fn new(contour: ContourDetector, hough: HoughDetector, config: HybridConfig) -> Self {
        Self {
            contour,
            hough,
            config,
        }
    }

    fn eligible(&self, candidate: &Detection) -> bool {
        candidate.confidence().is_finite()
            && candidate.confidence() >= self.config.minimum_confidence
            && candidate.metrics.edge_score >= self.config.minimum_edge_support
    }

    fn unambiguous(&self, ranked: &[Detection]) -> Option<Detection> {
        let best = *ranked.first()?;
        if let Some(second) = ranked.get(1)
            && best.confidence() - second.confidence() < self.config.minimum_candidate_margin
        {
            return None;
        }
        Some(best)
    }

    fn resolve(
        &self,
        contour: &[Detection],
        hough: &[Detection],
        long_side: f32,
    ) -> Option<Detection> {
        let distance = long_side * self.config.fusion_corner_distance_ratio;
        let mut supported = Vec::new();
        for (candidates, other_detector) in [(contour, hough), (hough, contour)] {
            for &candidate in candidates {
                let mut result = candidate;
                if !other_detector.is_empty() && distance > 0.0 {
                    let agreement = other_detector
                        .iter()
                        .map(|other| {
                            (1.0 - corner_distance(candidate.quad, other.quad) / distance)
                                .clamp(0.0, 1.0)
                        })
                        .fold(0.0, f32::max);
                    result.metrics.agreement_score = Some(agreement);
                    if agreement >= self.config.minimum_fusion_agreement {
                        // Fuse evidence, retaining a boundary scored on actual pixels.
                        result.source = DetectionSource::Fused;
                    }
                }
                if self.eligible(&result) {
                    supported.push(result);
                }
            }
        }
        let ranked = rank_distinct(supported, long_side * 0.02);
        self.unambiguous(&ranked)
    }
}

impl Detector<GrayImage> for HybridDetector {
    type Error = Infallible;

    /// None includes unresolved ambiguity as well as absence of a boundary.
    /// Confidence is heuristic evidence, not a calibrated probability.
    fn detect(&mut self, frame: &GrayImage) -> Result<Option<Detection>, Self::Error> {
        let contour: Vec<_> = self
            .contour
            .detect_candidates(frame)
            .into_iter()
            .filter(|candidate| self.eligible(candidate))
            .collect();
        if let Some(best) = self.unambiguous(&contour)
            && best.confidence() >= self.config.fast_accept_confidence
        {
            return Ok(Some(best));
        }
        let hough: Vec<_> = self
            .hough
            .detect_candidates(frame)
            .into_iter()
            .filter(|candidate| self.eligible(candidate))
            .collect();
        Ok(self.resolve(&contour, &hough, frame.width().max(frame.height()) as f32))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cirdean_core::{DetectionMetrics, Point, Quad};

    fn candidate(offset: f32, source: DetectionSource, score: f32) -> Detection {
        Detection {
            quad: Quad::new(
                Point::new(offset, 10.0),
                Point::new(offset + 100.0, 10.0),
                Point::new(offset + 100.0, 150.0),
                Point::new(offset, 150.0),
            ),
            source,
            metrics: DetectionMetrics {
                edge_score: score,
                geometry_score: score,
                ..Default::default()
            },
        }
    }

    #[test]
    fn high_confidence_competing_pages_are_not_fast_accepted() {
        let detector = HybridDetector::default();
        let pages = [
            candidate(10.0, DetectionSource::Contour, 0.98),
            candidate(180.0, DetectionSource::Contour, 0.97),
        ];
        assert!(detector.unambiguous(&pages).is_none());
        assert!(detector.resolve(&pages, &[], 500.0).is_none());
    }

    #[test]
    fn agreement_resolves_competition_without_moving_corners() {
        let detector = HybridDetector::default();
        let correct = candidate(10.0, DetectionSource::Contour, 0.85);
        let distractor = candidate(180.0, DetectionSource::Contour, 0.86);
        let hough = candidate(11.0, DetectionSource::Hough, 0.84);
        let found = detector
            .resolve(&[distractor, correct], &[hough], 500.0)
            .unwrap();
        assert_eq!(found.quad, correct.quad);
        assert_eq!(found.source, DetectionSource::Fused);
    }

    #[test]
    fn conflicting_detectors_abstain_on_a_near_tie() {
        let detector = HybridDetector::default();
        assert!(
            detector
                .resolve(
                    &[candidate(10.0, DetectionSource::Contour, 0.9)],
                    &[candidate(180.0, DetectionSource::Hough, 0.91)],
                    500.0
                )
                .is_none()
        );
    }

    #[test]
    fn perfect_geometry_cannot_rescue_missing_boundary_evidence() {
        let detector = HybridDetector::default();
        let mut weak = candidate(10.0, DetectionSource::Hough, 1.0);
        weak.metrics.edge_score = 0.4;
        assert!(detector.resolve(&[], &[weak], 500.0).is_none());
    }

    #[test]
    fn empty_frame_has_no_detection() {
        assert!(
            HybridDetector::default()
                .detect(&GrayImage::new(0, 0))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn small_preview_uses_resolution_scaled_hough_votes() {
        use image::Luma;
        use imageproc::{drawing::draw_polygon_mut, point::Point as ImagePoint};
        let mut frame = GrayImage::from_pixel(160, 120, Luma([20]));
        draw_polygon_mut(
            &mut frame,
            &[
                ImagePoint::new(16, 12),
                ImagePoint::new(144, 18),
                ImagePoint::new(140, 108),
                ImagePoint::new(10, 104),
            ],
            Luma([240]),
        );
        let found = HybridDetector::default()
            .detect(&frame)
            .unwrap()
            .expect("small document");
        let expected = Quad::new(
            Point::new(16.0, 12.0),
            Point::new(144.0, 18.0),
            Point::new(140.0, 108.0),
            Point::new(10.0, 104.0),
        );
        assert!(corner_distance(found.quad, expected) < 6.0);
    }

    #[test]
    fn clear_unique_contour_keeps_the_fast_path() {
        let mut frame = GrayImage::new(160, 160);
        for y in 15..145 {
            for x in 15..145 {
                frame.put_pixel(x, y, image::Luma([240]));
            }
        }
        let found = HybridDetector::default()
            .detect(&frame)
            .unwrap()
            .expect("clear document");
        assert_eq!(found.source, DetectionSource::Contour);
    }
}
