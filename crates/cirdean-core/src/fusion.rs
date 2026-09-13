use crate::{Detection, DetectionMetrics, DetectionSource, Point, Quad};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FusionConfig {
    /// Pixel distance at which two corresponding corners have zero agreement.
    pub max_corner_distance: f32,
    /// Reject fusion when the two detectors disagree more than this.
    pub minimum_agreement: f32,
}

impl Default for FusionConfig {
    fn default() -> Self {
        Self {
            max_corner_distance: 48.0,
            minimum_agreement: 0.50,
        }
    }
}

/// Compare corresponding corners and return a normalized `0.0..=1.0` score.
pub fn quad_agreement(a: Quad, b: Quad, max_corner_distance: f32) -> f32 {
    if max_corner_distance <= 0.0 {
        return 0.0;
    }

    let pairs = a.points().into_iter().zip(b.points());
    let mean_distance = pairs.map(|(left, right)| left.distance(right)).sum::<f32>() / 4.0;

    (1.0 - mean_distance / max_corner_distance).clamp(0.0, 1.0)
}

fn blend_point(a: Point, b: Point, weight_a: f32, weight_b: f32) -> Point {
    let total = weight_a + weight_b;
    if total <= f32::EPSILON {
        return Point::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
    }

    Point::new(
        (a.x * weight_a + b.x * weight_b) / total,
        (a.y * weight_a + b.y * weight_b) / total,
    )
}

/// Fuse two detector results when their quadrilaterals agree sufficiently.
///
/// Each detector's confidence becomes its geometric blend weight. The final
/// metrics preserve detector evidence while making cross-detector agreement an
/// explicit part of the result.
pub fn fuse_pair(first: Detection, second: Detection, config: FusionConfig) -> Option<Detection> {
    let agreement = quad_agreement(first.quad, second.quad, config.max_corner_distance);
    if agreement < config.minimum_agreement {
        return None;
    }

    let weight_first = first.confidence().max(0.001);
    let weight_second = second.confidence().max(0.001);
    let total = weight_first + weight_second;

    let a = first.quad.points();
    let b = second.quad.points();
    let quad = Quad::new(
        blend_point(a[0], b[0], weight_first, weight_second),
        blend_point(a[1], b[1], weight_first, weight_second),
        blend_point(a[2], b[2], weight_first, weight_second),
        blend_point(a[3], b[3], weight_first, weight_second),
    );

    let metrics = DetectionMetrics {
        edge_score: (first.metrics.edge_score * weight_first
            + second.metrics.edge_score * weight_second)
            / total,
        geometry_score: (first.metrics.geometry_score * weight_first
            + second.metrics.geometry_score * weight_second)
            / total,
        temporal_score: first
            .metrics
            .temporal_score
            .max(second.metrics.temporal_score),
        agreement_score: agreement,
    };

    Some(Detection {
        quad,
        source: DetectionSource::Fused,
        metrics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detection(offset: f32, source: DetectionSource) -> Detection {
        Detection {
            quad: Quad::new(
                Point::new(0.0 + offset, 0.0),
                Point::new(100.0 + offset, 0.0),
                Point::new(100.0 + offset, 200.0),
                Point::new(0.0 + offset, 200.0),
            ),
            source,
            metrics: DetectionMetrics {
                edge_score: 0.9,
                geometry_score: 0.9,
                temporal_score: 0.8,
                agreement_score: 0.0,
            },
        }
    }

    #[test]
    fn close_detections_are_fused() {
        let result = fuse_pair(
            detection(0.0, DetectionSource::Contour),
            detection(4.0, DetectionSource::Hough),
            FusionConfig::default(),
        )
        .expect("close detections should fuse");

        assert_eq!(result.source, DetectionSource::Fused);
        assert!(result.metrics.agreement_score > 0.8);
    }

    #[test]
    fn distant_detections_are_rejected() {
        let result = fuse_pair(
            detection(0.0, DetectionSource::Contour),
            detection(100.0, DetectionSource::Hough),
            FusionConfig::default(),
        );

        assert!(result.is_none());
    }
}
