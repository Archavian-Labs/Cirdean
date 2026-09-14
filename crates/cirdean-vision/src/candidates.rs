//! Ranking and spatial deduplication of document hypotheses.

use cirdean_core::{Detection, Quad};

/// Corner identity must not depend on which corner had the smallest x+y.
/// Both detectors produce clockwise quads, so cyclic alignment is sufficient.
pub(crate) fn corner_distance(first: Quad, second: Quad) -> f32 {
    let a = first.points();
    let b = second.points();
    (0..4)
        .map(|shift| {
            (0..4)
                .map(|index| a[index].distance(b[(index + shift) % 4]))
                .sum::<f32>()
                / 4.0
        })
        .fold(f32::INFINITY, f32::min)
}

/// Keep the best supported geometry for each spatial hypothesis. Do not average
/// corners: the averaged boundary has not been measured against the image.
pub(crate) fn rank_distinct(
    mut candidates: Vec<Detection>,
    duplicate_distance: f32,
) -> Vec<Detection> {
    candidates.sort_by(|a, b| b.confidence().total_cmp(&a.confidence()));
    let mut distinct: Vec<Detection> = Vec::new();
    for candidate in candidates {
        if !distinct
            .iter()
            .any(|other| corner_distance(candidate.quad, other.quad) <= duplicate_distance)
        {
            distinct.push(candidate);
        }
    }
    distinct
}

#[cfg(test)]
mod tests {
    use super::*;
    use cirdean_core::{DetectionMetrics, DetectionSource, Point};

    fn candidate(offset: f32, score: f32) -> Detection {
        Detection {
            quad: Quad::new(
                Point::new(offset, 0.0),
                Point::new(offset + 100.0, 0.0),
                Point::new(offset + 100.0, 100.0),
                Point::new(offset, 100.0),
            ),
            source: DetectionSource::Contour,
            metrics: DetectionMetrics {
                edge_score: score,
                geometry_score: score,
                ..Default::default()
            },
        }
    }

    #[test]
    fn repeated_views_do_not_hide_a_competing_document() {
        let ranked = rank_distinct(
            vec![
                candidate(1.0, 0.9),
                candidate(0.0, 0.95),
                candidate(60.0, 0.92),
            ],
            4.0,
        );
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].quad, candidate(0.0, 0.95).quad);
        assert_eq!(ranked[1].quad, candidate(60.0, 0.92).quad);
    }

    #[test]
    fn equivalent_rotated_corner_order_is_not_a_competitor() {
        let a = candidate(0.0, 0.9);
        let p = a.quad.points();
        let b = Detection {
            quad: Quad::new(p[1], p[2], p[3], p[0]),
            ..a
        };
        assert_eq!(rank_distinct(vec![a, b], 1.0).len(), 1);
    }
}
