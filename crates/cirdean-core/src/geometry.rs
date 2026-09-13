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
        let p = self.points();
        let mut twice_area = 0.0;
        for i in 0..4 {
            let a = p[i];
            let b = p[(i + 1) % 4];
            twice_area += a.x * b.y - b.x * a.y;
        }
        twice_area.abs() * 0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangle_area_is_correct() {
        let quad = Quad::new(
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 5.0),
            Point::new(0.0, 5.0),
        );
        assert_eq!(quad.area(), 50.0);
    }
}
