use std::convert::Infallible;

use cirdean_core::{Detection, Detector, FusionConfig, fuse_pair};
use image::GrayImage;

use crate::{contour::ContourDetector, hough::HoughDetector};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HybridConfig {
    /// Skip the Hough fallback when the cheap detector is already convincing.
    pub fast_accept_confidence: f32,
    /// Fusion distance normalized to the longest source-frame dimension.
    pub fusion_corner_distance_ratio: f32,
    pub minimum_fusion_agreement: f32,
}

impl Default for HybridConfig {
    fn default() -> Self {
        Self {
            fast_accept_confidence: 0.88,
            fusion_corner_distance_ratio: 0.04,
            minimum_fusion_agreement: 0.50,
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

    fn stronger(first: Detection, second: Detection) -> Detection {
        if first.confidence() >= second.confidence() {
            first
        } else {
            second
        }
    }
}

impl Detector<GrayImage> for HybridDetector {
    type Error = Infallible;

    fn detect(&mut self, frame: &GrayImage) -> Result<Option<Detection>, Self::Error> {
        let contour = self.contour.detect(frame)?;
        if contour
            .is_some_and(|detection| detection.confidence() >= self.config.fast_accept_confidence)
        {
            return Ok(contour);
        }

        let hough = self.hough.detect(frame)?;
        let result = match (contour, hough) {
            (None, None) => None,
            (Some(detection), None) | (None, Some(detection)) => Some(detection),
            (Some(contour), Some(hough)) => {
                let longest_side = frame.width().max(frame.height()) as f32;
                let fusion = FusionConfig {
                    max_corner_distance: longest_side * self.config.fusion_corner_distance_ratio,
                    minimum_agreement: self.config.minimum_fusion_agreement,
                };
                fuse_pair(contour, hough, fusion).or_else(|| Some(Self::stronger(contour, hough)))
            }
        };

        Ok(result)
    }
}
