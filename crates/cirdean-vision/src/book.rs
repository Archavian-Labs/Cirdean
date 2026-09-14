use image::GrayImage;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GutterEstimate {
    pub x: u32,
    pub confidence: f32,
}

/// Hbot-inspired gutter estimator with an explicit confidence value.
///
/// The darkest smoothed vertical band in the central 35-65% of the spread is a
/// useful candidate for the binding shadow, but it should not be treated as
/// truth when the center has little luminance contrast. A broad dark gutter can
/// create a flat minimum after smoothing, so Cirdean returns the center of that
/// minimum plateau instead of whichever edge happens to compare first.
pub fn estimate_gutter(image: &GrayImage) -> Option<GutterEstimate> {
    let (width, height) = image.dimensions();
    if width < 20 || height == 0 {
        return None;
    }

    let means: Vec<f32> = (0..width)
        .map(|x| {
            (0..height)
                .map(|y| image.get_pixel(x, y)[0] as f32)
                .sum::<f32>()
                / height as f32
        })
        .collect();

    let radius = 12_i32;
    let smooth: Vec<f32> = (0..width)
        .map(|x| {
            let start = (x as i32 - radius).max(0) as u32;
            let end = (x as i32 + radius).min(width as i32 - 1) as u32;
            let count = end - start + 1;
            (start..=end)
                .map(|sample| means[sample as usize])
                .sum::<f32>()
                / count as f32
        })
        .collect();

    let low = ((width as f32 * 0.35).round() as u32).min(width - 1);
    let high = ((width as f32 * 0.65).round() as u32)
        .max(low + 1)
        .min(width);
    let search = &smooth[low as usize..high as usize];
    let (minimum_index, minimum) = search
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| left.total_cmp(right))?;

    // Sliding-window smoothing often turns a wide binding shadow into a flat
    // minimum. Expand around the first minimum and use the plateau midpoint so
    // the estimate stays centered on the binding rather than biased left/right.
    let tolerance = minimum.abs().max(1.0) * 1.0e-5;
    let mut plateau_left = minimum_index;
    while plateau_left > 0 && (search[plateau_left - 1] - *minimum).abs() <= tolerance {
        plateau_left -= 1;
    }

    let mut plateau_right = minimum_index;
    while plateau_right + 1 < search.len()
        && (search[plateau_right + 1] - *minimum).abs() <= tolerance
    {
        plateau_right += 1;
    }

    let relative_index = (plateau_left + plateau_right) / 2;
    let mean = search.iter().sum::<f32>() / search.len() as f32;
    let confidence = if mean <= f32::EPSILON {
        0.0
    } else {
        ((mean - *minimum) / mean).clamp(0.0, 1.0)
    };

    Some(GutterEstimate {
        x: low + relative_index as u32,
        confidence,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Luma;

    #[test]
    fn finds_off_center_dark_gutter() {
        let mut image = GrayImage::from_pixel(300, 200, Luma([245]));
        for x in 126..=134 {
            for y in 0..image.height() {
                image.put_pixel(x, y, Luma([20]));
            }
        }

        let estimate = estimate_gutter(&image).expect("gutter should be estimated");
        assert!((estimate.x as i32 - 130).abs() <= 2);
        assert!(estimate.confidence > 0.15);
    }

    #[test]
    fn uniform_center_has_no_gutter_confidence() {
        let image = GrayImage::from_pixel(300, 200, Luma([180]));
        let estimate = estimate_gutter(&image).expect("geometry still allows an estimate");
        assert!(estimate.confidence <= f32::EPSILON);
    }
}
