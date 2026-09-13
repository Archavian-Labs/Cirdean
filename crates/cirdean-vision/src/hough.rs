use std::{collections::BTreeSet, convert::Infallible};

use cirdean_core::{Detection, DetectionMetrics, DetectionSource, Detector, Point};
use image::GrayImage;
use imageproc::hough::{LineDetectionOptions, PolarLine, detect_lines};

use crate::{
    preprocess::{downscale_long_side, hough_edge_map},
    scoring::{edge_support, geometry_score, order_quad, scale_quad},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoughConfig {
    pub preview_long_side: u32,
    pub vote_thresholds: [u32; 3],
    pub suppression_radius: u32,
    pub maximum_lines: usize,
    pub minimum_intersection_angle_degrees: f32,
    pub minimum_corner_distance_ratio: f32,
    pub minimum_area_ratio: f32,
    pub maximum_cycles: usize,
}

impl Default for HoughConfig {
    fn default() -> Self {
        Self {
            preview_long_side: 500,
            vote_thresholds: [100, 150, 200],
            suppression_radius: 4,
            maximum_lines: 16,
            minimum_intersection_angle_degrees: 45.0,
            minimum_corner_distance_ratio: 0.08,
            minimum_area_ratio: 0.18,
            maximum_cycles: 512,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Intersection {
    point: Point,
    source_lines: (usize, usize),
}

fn angle_difference_degrees(first: PolarLine, second: PolarLine) -> f32 {
    let difference = (first.angle_in_degrees as f32 - second.angle_in_degrees as f32).abs();
    difference.min(180.0 - difference)
}

fn intersect_lines(first: PolarLine, second: PolarLine) -> Option<Point> {
    let first_angle = (first.angle_in_degrees as f32).to_radians();
    let second_angle = (second.angle_in_degrees as f32).to_radians();
    let (first_sin, first_cos) = first_angle.sin_cos();
    let (second_sin, second_cos) = second_angle.sin_cos();
    let determinant = first_cos * second_sin - first_sin * second_cos;

    if determinant.abs() <= 1.0e-5 {
        return None;
    }

    let x = (first.r * second_sin - first_sin * second.r) / determinant;
    let y = (first_cos * second.r - first.r * second_cos) / determinant;
    Some(Point::new(x, y))
}

fn bounded_lines(edge_map: &GrayImage, config: HoughConfig) -> Vec<PolarLine> {
    let mut last_lines = Vec::new();

    for threshold in config.vote_thresholds {
        let lines = detect_lines(
            edge_map,
            LineDetectionOptions {
                vote_threshold: threshold,
                suppression_radius: config.suppression_radius,
            },
        );
        if lines.len() <= config.maximum_lines {
            return lines;
        }
        last_lines = lines;
    }

    let mut threshold = config.vote_thresholds[2].saturating_add(50);
    let ceiling = edge_map.width().max(edge_map.height()).saturating_mul(2);
    while last_lines.len() > config.maximum_lines && threshold <= ceiling {
        last_lines = detect_lines(
            edge_map,
            LineDetectionOptions {
                vote_threshold: threshold,
                suppression_radius: config.suppression_radius,
            },
        );
        threshold = threshold.saturating_add(50);
    }

    last_lines.truncate(config.maximum_lines);
    last_lines
}

fn find_intersections(
    lines: &[PolarLine],
    width: u32,
    height: u32,
    minimum_angle_degrees: f32,
) -> Vec<Intersection> {
    let mut intersections = Vec::new();

    for first_index in 0..lines.len() {
        for second_index in (first_index + 1)..lines.len() {
            let first = lines[first_index];
            let second = lines[second_index];
            if angle_difference_degrees(first, second) < minimum_angle_degrees {
                continue;
            }

            let Some(point) = intersect_lines(first, second) else {
                continue;
            };
            if point.x < 0.0 || point.y < 0.0 || point.x > width as f32 || point.y > height as f32 {
                continue;
            }

            intersections.push(Intersection {
                point,
                source_lines: (first_index, second_index),
            });
        }
    }

    intersections
}

fn shares_line(first: Intersection, second: Intersection) -> bool {
    first.source_lines.0 == second.source_lines.0
        || first.source_lines.0 == second.source_lines.1
        || first.source_lines.1 == second.source_lines.0
        || first.source_lines.1 == second.source_lines.1
}

fn build_graph(intersections: &[Intersection], minimum_corner_distance: f32) -> Vec<Vec<usize>> {
    let mut graph = vec![Vec::new(); intersections.len()];

    for first in 0..intersections.len() {
        for second in (first + 1)..intersections.len() {
            if shares_line(intersections[first], intersections[second])
                && intersections[first]
                    .point
                    .distance(intersections[second].point)
                    >= minimum_corner_distance
            {
                graph[first].push(second);
                graph[second].push(first);
            }
        }
    }

    graph
}

fn canonical_cycle(cycle: [usize; 4]) -> [usize; 4] {
    let mut candidates = Vec::with_capacity(8);
    for shift in 0..4 {
        candidates.push([
            cycle[shift],
            cycle[(shift + 1) % 4],
            cycle[(shift + 2) % 4],
            cycle[(shift + 3) % 4],
        ]);
        candidates.push([
            cycle[shift],
            cycle[(shift + 3) % 4],
            cycle[(shift + 2) % 4],
            cycle[(shift + 1) % 4],
        ]);
    }
    candidates.into_iter().min().expect("cycle has candidates")
}

fn enumerate_cycles(graph: &[Vec<usize>], maximum_cycles: usize) -> Vec<[usize; 4]> {
    fn visit(
        graph: &[Vec<usize>],
        start: usize,
        path: &mut Vec<usize>,
        output: &mut BTreeSet<[usize; 4]>,
        maximum_cycles: usize,
    ) {
        if output.len() >= maximum_cycles {
            return;
        }

        let current = *path.last().expect("path is never empty");
        if path.len() == 4 {
            if graph[current].contains(&start) {
                output.insert(canonical_cycle([path[0], path[1], path[2], path[3]]));
            }
            return;
        }

        for &next in &graph[current] {
            if !path.contains(&next) {
                path.push(next);
                visit(graph, start, path, output, maximum_cycles);
                path.pop();
                if output.len() >= maximum_cycles {
                    return;
                }
            }
        }
    }

    let mut cycles = BTreeSet::new();
    for start in 0..graph.len() {
        let mut path = vec![start];
        visit(graph, start, &mut path, &mut cycles, maximum_cycles);
        if cycles.len() >= maximum_cycles {
            break;
        }
    }

    cycles.into_iter().collect()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoughDetector {
    config: HoughConfig,
}

impl HoughDetector {
    pub const fn new(config: HoughConfig) -> Self {
        Self { config }
    }
}

impl Default for HoughDetector {
    fn default() -> Self {
        Self::new(HoughConfig::default())
    }
}

impl Detector<GrayImage> for HoughDetector {
    type Error = Infallible;

    fn detect(&mut self, frame: &GrayImage) -> Result<Option<Detection>, Self::Error> {
        let scaled = downscale_long_side(frame, self.config.preview_long_side);
        let edge_map = hough_edge_map(&scaled.image);
        let lines = bounded_lines(&edge_map, self.config);
        let intersections = find_intersections(
            &lines,
            edge_map.width(),
            edge_map.height(),
            self.config.minimum_intersection_angle_degrees,
        );
        let minimum_corner_distance = edge_map.width().max(edge_map.height()) as f32
            * self.config.minimum_corner_distance_ratio;
        let graph = build_graph(&intersections, minimum_corner_distance);
        let cycles = enumerate_cycles(&graph, self.config.maximum_cycles);

        let best = cycles
            .into_iter()
            .filter_map(|cycle| {
                let quad = order_quad([
                    intersections[cycle[0]].point,
                    intersections[cycle[1]].point,
                    intersections[cycle[2]].point,
                    intersections[cycle[3]].point,
                ])?;
                let geometry = geometry_score(
                    quad,
                    edge_map.width(),
                    edge_map.height(),
                    self.config.minimum_area_ratio,
                );
                if geometry <= f32::EPSILON {
                    return None;
                }

                let edges = edge_support(&edge_map, quad, 2);
                Some(Detection {
                    quad,
                    source: DetectionSource::Hough,
                    metrics: DetectionMetrics {
                        edge_score: edges,
                        geometry_score: geometry,
                        temporal_score: None,
                        agreement_score: None,
                    },
                })
            })
            .max_by(|left, right| left.confidence().total_cmp(&right.confidence()));

        Ok(best.map(|mut detection| {
            detection.quad =
                scale_quad(detection.quad, scaled.source_scale_x, scaled.source_scale_y);
            detection
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polar_lines_intersect_in_cartesian_space() {
        let vertical = PolarLine {
            r: 20.0,
            angle_in_degrees: 0,
        };
        let horizontal = PolarLine {
            r: 30.0,
            angle_in_degrees: 90,
        };
        let point = intersect_lines(vertical, horizontal).expect("lines should intersect");
        assert!((point.x - 20.0).abs() < 1.0e-4);
        assert!((point.y - 30.0).abs() < 1.0e-4);
    }

    #[test]
    fn equivalent_cycles_have_one_canonical_form() {
        let first = canonical_cycle([0, 1, 2, 3]);
        let rotated = canonical_cycle([2, 3, 0, 1]);
        let reversed = canonical_cycle([0, 3, 2, 1]);
        assert_eq!(first, rotated);
        assert_eq!(first, reversed);
    }

    #[test]
    fn cycle_enumeration_is_bounded() {
        let graph = vec![
            vec![1, 2, 3, 4],
            vec![0, 2, 3, 4],
            vec![0, 1, 3, 4],
            vec![0, 1, 2, 4],
            vec![0, 1, 2, 3],
        ];
        assert!(enumerate_cycles(&graph, 3).len() <= 3);
    }
}
