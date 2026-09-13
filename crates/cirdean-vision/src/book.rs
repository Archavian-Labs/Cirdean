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
/// truth when the center has little luminance contrast.
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
    let (relative_index, minimum) = search
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| left.total_cmp(right))?;
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
        assert!((estimate.x as i32 - 130).abs() <= 6);
        assert!(estimate.confidence > 0.15);
    }
}
