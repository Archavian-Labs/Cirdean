use cirdean_core::{Point, Quad};
use image::GrayImage;

pub fn order_quad(points: [Point; 4]) -> Option<Quad> {
    let center = Point::new(
        points.iter().map(|point| point.x).sum::<f32>() / 4.0,
        points.iter().map(|point| point.y).sum::<f32>() / 4.0,
    );

    let mut ordered = points;
    ordered.sort_by(|left, right| {
        let left_angle = (left.y - center.y).atan2(left.x - center.x);
        let right_angle = (right.y - center.y).atan2(right.x - center.x);
        left_angle.total_cmp(&right_angle)
    });

    let start = ordered
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| (left.x + left.y).total_cmp(&(right.x + right.y)))
        .map(|(index, _)| index)?;
    ordered.rotate_left(start);

    let quad = Quad::new(ordered[0], ordered[1], ordered[2], ordered[3]);
    quad.is_convex().then_some(quad)
}

pub fn scale_quad(quad: Quad, scale_x: f32, scale_y: f32) -> Quad {
    let scale = |point: Point| Point::new(point.x * scale_x, point.y * scale_y);
    Quad::new(
        scale(quad.top_left),
        scale(quad.top_right),
        scale(quad.bottom_right),
        scale(quad.bottom_left),
    )
}

fn ratio_similarity(left: f32, right: f32) -> f32 {
    let largest = left.max(right);
    if largest <= f32::EPSILON {
        0.0
    } else {
        left.min(right) / largest
    }
}

fn orthogonality_score(quad: Quad) -> f32 {
    let points = quad.points();
    let mut score = 0.0;

    for index in 0..4 {
        let previous = points[(index + 3) % 4];
        let current = points[index];
        let next = points[(index + 1) % 4];
        let a = (previous.x - current.x, previous.y - current.y);
        let b = (next.x - current.x, next.y - current.y);
        let length_a = (a.0 * a.0 + a.1 * a.1).sqrt();
        let length_b = (b.0 * b.0 + b.1 * b.1).sqrt();

        if length_a <= f32::EPSILON || length_b <= f32::EPSILON {
            return 0.0;
        }

        let cosine = (a.0 * b.0 + a.1 * b.1) / (length_a * length_b);
        score += 1.0 - cosine.abs().clamp(0.0, 1.0);
    }

    score / 4.0
}

/// Score document-like geometry without assuming a perfectly front-facing page.
pub fn geometry_score(
    quad: Quad,
    frame_width: u32,
    frame_height: u32,
    minimum_area_ratio: f32,
) -> f32 {
    if !quad.is_convex() || frame_width == 0 || frame_height == 0 {
        return 0.0;
    }

    let frame_area = frame_width as f32 * frame_height as f32;
    let area_ratio = quad.area() / frame_area;
    if area_ratio < minimum_area_ratio {
        return 0.0;
    }

    let area_score = (area_ratio / 0.65).clamp(0.0, 1.0);
    let edges = quad.edge_lengths();
    let opposite_balance =
        (ratio_similarity(edges[0], edges[2]) + ratio_similarity(edges[1], edges[3])) * 0.5;
    let right_angle_score = orthogonality_score(quad);

    (0.45 * area_score + 0.30 * right_angle_score + 0.25 * opposite_balance).clamp(0.0, 1.0)
}

fn has_edge_near(edge_map: &GrayImage, x: i32, y: i32, radius: i32) -> bool {
    let width = edge_map.width() as i32;
    let height = edge_map.height() as i32;

    for offset_y in -radius..=radius {
        for offset_x in -radius..=radius {
            let sample_x = x + offset_x;
            let sample_y = y + offset_y;
            if sample_x >= 0
                && sample_x < width
                && sample_y >= 0
                && sample_y < height
                && edge_map.get_pixel(sample_x as u32, sample_y as u32)[0] > 0
            {
                return true;
            }
        }
    }

    false
}

/// Fraction of sampled quadrilateral boundary positions supported by edge pixels.
pub fn edge_support(edge_map: &GrayImage, quad: Quad, search_radius: i32) -> f32 {
    let points = quad.points();
    let mut supported = 0_u32;
    let mut samples = 0_u32;

    for index in 0..4 {
        let start = points[index];
        let end = points[(index + 1) % 4];
        let distance = start.distance(end);
        let steps = (distance.ceil() as usize).clamp(8, 256);

        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let x = start.x + (end.x - start.x) * t;
            let y = start.y + (end.y - start.y) * t;
            samples += 1;
            if has_edge_near(edge_map, x.round() as i32, y.round() as i32, search_radius) {
                supported += 1;
            }
        }
    }

    if samples == 0 {
        0.0
    } else {
        supported as f32 / samples as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Luma;
    use imageproc::{drawing::draw_line_segment_mut, point::Point as ImagePoint};

    #[test]
    fn geometry_prefers_large_regular_document() {
        let quad = order_quad([
            Point::new(20.0, 20.0),
            Point::new(180.0, 20.0),
            Point::new(180.0, 280.0),
            Point::new(20.0, 280.0),
        ])
        .expect("rectangle should order");
        assert!(geometry_score(quad, 200, 300, 0.15) > 0.75);
    }

    #[test]
    fn edge_support_recognizes_quad_boundary() {
        let mut image = GrayImage::new(120, 120);
        let points = [
            ImagePoint::new(10_i32, 10_i32),
            ImagePoint::new(110_i32, 10_i32),
            ImagePoint::new(110_i32, 110_i32),
            ImagePoint::new(10_i32, 110_i32),
        ];
        for index in 0..4 {
            let a = points[index];
            let b = points[(index + 1) % 4];
            draw_line_segment_mut(
                &mut image,
                (a.x as f32, a.y as f32),
                (b.x as f32, b.y as f32),
                Luma([255]),
            );
        }
        let quad = Quad::new(
            Point::new(10.0, 10.0),
            Point::new(110.0, 10.0),
            Point::new(110.0, 110.0),
            Point::new(10.0, 110.0),
        );
        assert!(edge_support(&image, quad, 1) > 0.95);
    }
}
