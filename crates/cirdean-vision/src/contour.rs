use std::convert::Infallible;

use cirdean_core::{Detection, DetectionMetrics, DetectionSource, Detector, Point};
use image::GrayImage;
use imageproc::{
    contours::{BorderType, find_contours},
    geometry::{approximate_polygon_dp, arc_length, contour_area},
};

use crate::{
    candidates::rank_distinct,
    preprocess::{contour_edge_maps, downscale_long_side},
    scoring::{edge_support, geometry_score, order_quad, scale_quad},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContourConfig {
    pub preview_long_side: u32,
    pub minimum_area_ratio: f32,
    pub approximation_epsilon_ratio: f64,
    pub maximum_candidates_per_map: usize,
}

impl Default for ContourConfig {
    fn default() -> Self {
        Self {
            preview_long_side: 700,
            minimum_area_ratio: 0.15,
            approximation_epsilon_ratio: 0.02,
            maximum_candidates_per_map: 8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContourDetector {
    config: ContourConfig,
}

impl ContourDetector {
    pub const fn new(config: ContourConfig) -> Self {
        Self { config }
    }

    fn detect_on_map(&self, edge_map: &GrayImage) -> Vec<Detection> {
        let mut contours = find_contours::<i32>(edge_map);
        contours.retain(|contour| {
            contour.border_type == BorderType::Outer && contour.points.len() >= 4
        });
        contours.sort_by(|left, right| {
            contour_area(&right.points).total_cmp(&contour_area(&left.points))
        });

        contours
            .into_iter()
            .take(self.config.maximum_candidates_per_map)
            .filter_map(|contour| {
                let perimeter = arc_length(&contour.points, true);
                if perimeter <= f64::EPSILON {
                    return None;
                }

                let approximated = approximate_polygon_dp(
                    &contour.points,
                    perimeter * self.config.approximation_epsilon_ratio,
                    true,
                );
                if approximated.len() != 4 {
                    return None;
                }

                let points = [
                    Point::new(approximated[0].x as f32, approximated[0].y as f32),
                    Point::new(approximated[1].x as f32, approximated[1].y as f32),
                    Point::new(approximated[2].x as f32, approximated[2].y as f32),
                    Point::new(approximated[3].x as f32, approximated[3].y as f32),
                ];
                let quad = order_quad(points)?;
                let geometry = geometry_score(
                    quad,
                    edge_map.width(),
                    edge_map.height(),
                    self.config.minimum_area_ratio,
                );
                if geometry <= f32::EPSILON {
                    return None;
                }

                let edges = edge_support(edge_map, quad, 2);
                Some(Detection {
                    quad,
                    source: DetectionSource::Contour,
                    metrics: DetectionMetrics {
                        edge_score: edges,
                        geometry_score: geometry,
                        temporal_score: None,
                        agreement_score: None,
                    },
                })
            })
            .collect()
    }

    /// Ranked, distinct hypotheses in source coordinates, retained so the
    /// hybrid router can distinguish high confidence from low ambiguity.
    pub fn detect_candidates(&self, frame: &GrayImage) -> Vec<Detection> {
        if frame.width() < 3 || frame.height() < 3 {
            return Vec::new();
        }
        let scaled = downscale_long_side(frame, self.config.preview_long_side);
        let maps = contour_edge_maps(&scaled.image);
        let candidates = maps
            .iter()
            .flat_map(|edge_map| self.detect_on_map(edge_map))
            .map(|mut detection| {
                detection.quad =
                    scale_quad(detection.quad, scaled.source_scale_x, scaled.source_scale_y);
                detection
            })
            .collect();
        rank_distinct(candidates, frame.width().max(frame.height()) as f32 * 0.02)
    }
}

impl Default for ContourDetector {
    fn default() -> Self {
        Self::new(ContourConfig::default())
    }
}

impl Detector<GrayImage> for ContourDetector {
    type Error = Infallible;

    fn detect(&mut self, frame: &GrayImage) -> Result<Option<Detection>, Self::Error> {
        Ok(self.detect_candidates(frame).into_iter().next())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Luma;
    use imageproc::{drawing::draw_polygon_mut, point::Point as ImagePoint};

    #[test]
    fn detects_synthetic_document() {
        let mut image = GrayImage::from_pixel(640, 480, Luma([15]));
        draw_polygon_mut(
            &mut image,
            &[
                ImagePoint::new(90, 50),
                ImagePoint::new(550, 70),
                ImagePoint::new(530, 430),
                ImagePoint::new(70, 410),
            ],
            Luma([240]),
        );

        let detection = ContourDetector::default()
            .detect(&image)
            .expect("detector is infallible")
            .expect("synthetic page should be detected");

        assert_eq!(detection.source, DetectionSource::Contour);
        assert!(detection.quad.area() > 100_000.0);
        assert!(detection.confidence() > 0.60);
    }
}
