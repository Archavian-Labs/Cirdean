/// A two-dimensional point in image coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn distance(self, other: Self) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// Ordered document corners: top-left, top-right, bottom-right, bottom-left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quad {
    pub top_left: Point,
    pub top_right: Point,
    pub bottom_right: Point,
    pub bottom_left: Point,
}

impl Quad {
    pub const fn new(
        top_left: Point,
        top_right: Point,
        bottom_right: Point,
        bottom_left: Point,
    ) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    pub fn points(self) -> [Point; 4] {
        [
            self.top_left,
            self.top_right,
            self.bottom_right,
            self.bottom_left,
        ]
    }

    /// Shoelace area. Useful for rejecting tiny or degenerate candidates.
    pub fn area(self) -> f32 {
        let points = self.points();
        let mut twice_area = 0.0;
        for index in 0..4 {
            let current = points[index];
            let next = points[(index + 1) % 4];
            twice_area += current.x * next.y - next.x * current.y;
        }
        twice_area.abs() * 0.5
    }

    /// Mean of the four corners, useful for lightweight temporal tracking.
    pub fn centroid(self) -> Point {
        let points = self.points();
        Point::new(
            points.iter().map(|point| point.x).sum::<f32>() / 4.0,
            points.iter().map(|point| point.y).sum::<f32>() / 4.0,
        )
    }

    pub fn edge_lengths(self) -> [f32; 4] {
        let points = self.points();
        [
            points[0].distance(points[1]),
            points[1].distance(points[2]),
            points[2].distance(points[3]),
            points[3].distance(points[0]),
        ]
    }

    /// Mean distance between corresponding ordered corners.
    pub fn average_corner_distance(self, other: Self) -> f32 {
        self.points()
            .into_iter()
            .zip(other.points())
            .map(|(left, right)| left.distance(right))
            .sum::<f32>()
            / 4.0
    }

    /// Reject self-intersecting or degenerate quadrilaterals.
    pub fn is_convex(self) -> bool {
        let points = self.points();
        let mut orientation = 0.0_f32;

        for index in 0..4 {
            let a = points[index];
            let b = points[(index + 1) % 4];
            let c = points[(index + 2) % 4];
            let ab = (b.x - a.x, b.y - a.y);
            let bc = (c.x - b.x, c.y - b.y);
            let cross = ab.0 * bc.1 - ab.1 * bc.0;

            if cross.abs() <= f32::EPSILON {
                return false;
            }

            if orientation == 0.0 {
                orientation = cross.signum();
            } else if cross.signum() != orientation {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rectangle() -> Quad {
        Quad::new(
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 5.0),
            Point::new(0.0, 5.0),
        )
    }

    #[test]
    fn rectangle_area_is_correct() {
        assert_eq!(rectangle().area(), 50.0);
    }

    #[test]
    fn rectangle_geometry_is_stable() {
        let quad = rectangle();
        assert_eq!(quad.centroid(), Point::new(5.0, 2.5));
        assert_eq!(quad.edge_lengths(), [10.0, 5.0, 10.0, 5.0]);
        assert!(quad.is_convex());
    }

    #[test]
    fn corner_distance_tracks_quad_motion() {
        let first = rectangle();
        let second = Quad::new(
            Point::new(4.0, 0.0),
            Point::new(14.0, 0.0),
            Point::new(14.0, 5.0),
            Point::new(4.0, 5.0),
        );
        assert_eq!(first.average_corner_distance(second), 4.0);
    }

    #[test]
    fn crossed_quad_is_not_convex() {
        let crossed = Quad::new(
            Point::new(0.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
            Point::new(10.0, 0.0),
        );
        assert!(!crossed.is_convex());
    }
}
