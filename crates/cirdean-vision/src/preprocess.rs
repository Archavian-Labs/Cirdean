use image::{GrayImage, imageops::FilterType};
use imageproc::{
    contrast::adaptive_threshold,
    distance_transform::Norm,
    edges::canny,
    filter::gaussian_blur_f32,
    morphology::{Mask, dilate, grayscale_close},
};

#[derive(Debug, Clone)]
pub struct ScaledFrame {
    pub image: GrayImage,
    pub source_scale_x: f32,
    pub source_scale_y: f32,
}

pub fn downscale_long_side(source: &GrayImage, maximum_long_side: u32) -> ScaledFrame {
    let (source_width, source_height) = source.dimensions();
    let long_side = source_width.max(source_height);

    if maximum_long_side == 0 || long_side <= maximum_long_side {
        return ScaledFrame {
            image: source.clone(),
            source_scale_x: 1.0,
            source_scale_y: 1.0,
        };
    }

    let scale = maximum_long_side as f32 / long_side as f32;
    let width = ((source_width as f32 * scale).round() as u32).max(1);
    let height = ((source_height as f32 * scale).round() as u32).max(1);
    let image = image::imageops::resize(source, width, height, FilterType::Triangle);

    ScaledFrame {
        source_scale_x: source_width as f32 / width as f32,
        source_scale_y: source_height as f32 / height as f32,
        image,
    }
}

/// Hbot-inspired fast preprocessing. The two views are intentionally different:
/// ordinary Canny works well on clean borders, while adaptive thresholding can
/// recover a page whose boundary has poor global contrast.
pub fn contour_edge_maps(gray: &GrayImage) -> [GrayImage; 2] {
    let blurred = gaussian_blur_f32(gray, 1.2);

    let ordinary = canny(&blurred, 50.0, 150.0);
    let ordinary = dilate(&ordinary, Norm::LInf, 1);

    // A radius of 10 corresponds to a 21x21 local neighborhood, mirroring the
    // scale of Hbot's adaptive-threshold path.
    let adaptive = adaptive_threshold(&blurred, 10, 5);
    let adaptive = canny(&adaptive, 50.0, 150.0);

    [ordinary, adaptive]
}

/// Camscan-inspired preprocessing for the expensive Hough fallback.
pub fn hough_edge_map(gray: &GrayImage) -> GrayImage {
    let blurred = gaussian_blur_f32(gray, 3.0);
    // Radius 6 gives a 13x13 grayscale morphological neighborhood, matching the
    // broad structural cleanup used by Camscan before Canny/Hough processing.
    let closed = grayscale_close(&blurred, &Mask::square(6));
    canny(&closed, 0.0, 84.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downscale_preserves_coordinate_mapping() {
        let image = GrayImage::new(2_000, 1_000);
        let scaled = downscale_long_side(&image, 500);
        assert_eq!(scaled.image.dimensions(), (500, 250));
        assert_eq!(scaled.source_scale_x, 4.0);
        assert_eq!(scaled.source_scale_y, 4.0);
    }
}
