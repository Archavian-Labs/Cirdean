//! Deterministic, synthetic comparison of the three document detectors.
//!
//! Run with `cargo run -p cirdean-vision --release --example compare_detectors`.

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    time::{Duration, Instant},
};

use cirdean_core::{Detector, Point, Quad};
use cirdean_vision::{ContourDetector, HoughDetector, HybridDetector};
use image::{GrayImage, Luma};
use imageproc::{
    drawing::{draw_line_segment_mut, draw_polygon_mut},
    point::Point as ImagePoint,
};

const WIDTH: u32 = 640;
const HEIGHT: u32 = 480;
const RUNS: usize = 3;

#[derive(Clone, Copy)]
enum Expected {
    Quad(Quad),
    None,
    Ambiguous,
}

struct Case {
    name: &'static str,
    image: GrayImage,
    expected: Expected,
}

fn quad(points: [(i32, i32); 4]) -> Quad {
    Quad::new(
        Point::new(points[0].0 as f32, points[0].1 as f32),
        Point::new(points[1].0 as f32, points[1].1 as f32),
        Point::new(points[2].0 as f32, points[2].1 as f32),
        Point::new(points[3].0 as f32, points[3].1 as f32),
    )
}

fn document(background: u8, page: u8, points: [(i32, i32); 4]) -> GrayImage {
    let mut image = GrayImage::from_pixel(WIDTH, HEIGHT, Luma([background]));
    let polygon = points.map(|(x, y)| ImagePoint::new(x, y));
    draw_polygon_mut(&mut image, &polygon, Luma([page]));
    image
}

fn small_document() -> (GrayImage, Quad) {
    let points = [(16, 12), (144, 18), (140, 108), (10, 104)];
    let mut image = GrayImage::from_pixel(160, 120, Luma([20]));
    let polygon = points.map(|(x, y)| ImagePoint::new(x, y));
    draw_polygon_mut(&mut image, &polygon, Luma([240]));
    (image, quad(points))
}

fn outline(image: &mut GrayImage, points: [(i32, i32); 4], value: u8, width: i32) {
    for offset in -width..=width {
        for index in 0..4 {
            let (x1, y1) = points[index];
            let (x2, y2) = points[(index + 1) % 4];
            draw_line_segment_mut(
                image,
                ((x1 + offset) as f32, (y1 + offset) as f32),
                ((x2 + offset) as f32, (y2 + offset) as f32),
                Luma([value]),
            );
        }
    }
}

fn clutter(image: &mut GrayImage) {
    for x in (20..WIDTH as i32).step_by(37) {
        draw_line_segment_mut(
            image,
            (x as f32, 0.0),
            (x as f32, HEIGHT as f32),
            Luma([180]),
        );
    }
    for y in (15..HEIGHT as i32).step_by(31) {
        draw_line_segment_mut(
            image,
            (0.0, y as f32),
            (WIDTH as f32, y as f32),
            Luma([180]),
        );
    }
}

fn cases() -> Vec<Case> {
    let clean = [(90, 50), (550, 70), (530, 430), (70, 410)];
    let low_contrast = [(100, 55), (545, 78), (525, 425), (78, 405)];
    let broken = [(85, 60), (555, 75), (525, 425), (65, 400)];
    let (small_image, small_truth) = small_document();
    let mut broken_image = document(20, 235, broken);
    // Erase four deterministic gaps from the border while preserving a filled page.
    for (x, y) in [(300, 68), (520, 250), (270, 415), (72, 220)] {
        for dx in 0..18 {
            for dy in 0..8 {
                if x + dx < WIDTH && y + dy < HEIGHT {
                    broken_image.put_pixel(x + dx, y + dy, Luma([20]));
                }
            }
        }
    }
    let mut clutter_image = GrayImage::from_pixel(WIDTH, HEIGHT, Luma([35]));
    clutter(&mut clutter_image);
    let mut nested_image = document(20, 220, clean);
    outline(
        &mut nested_image,
        [(145, 105), (480, 115), (470, 365), (135, 350)],
        250,
        2,
    );

    vec![
        Case {
            name: "clean_skewed",
            image: document(20, 240, clean),
            expected: Expected::Quad(quad(clean)),
        },
        Case {
            name: "low_contrast",
            image: document(90, 150, low_contrast),
            expected: Expected::Quad(quad(low_contrast)),
        },
        Case {
            name: "broken_outline",
            image: broken_image,
            expected: Expected::Quad(quad(broken)),
        },
        Case {
            name: "clutter_grid_no_document",
            image: clutter_image,
            expected: Expected::None,
        },
        Case {
            name: "blank",
            image: GrayImage::from_pixel(WIDTH, HEIGHT, Luma([100])),
            expected: Expected::None,
        },
        Case {
            name: "nested_rectangles",
            image: nested_image,
            expected: Expected::Ambiguous,
        },
        Case {
            name: "small_resolution",
            image: small_image,
            expected: Expected::Quad(small_truth),
        },
    ]
}

fn run(kind: &str, image: &GrayImage) -> (Option<cirdean_core::Detection>, Duration, bool) {
    let start = Instant::now();
    let result = catch_unwind(AssertUnwindSafe(|| match kind {
        "contour" => ContourDetector::default()
            .detect(image)
            .expect("infallible detector"),
        "hough" => HoughDetector::default()
            .detect(image)
            .expect("infallible detector"),
        "hybrid" => HybridDetector::default()
            .detect(image)
            .expect("infallible detector"),
        _ => unreachable!("known detector"),
    }));
    match result {
        Ok(detection) => (detection, start.elapsed(), false),
        Err(_) => (None, start.elapsed(), true),
    }
}

fn median_micros(times: &mut [Duration]) -> u128 {
    times.sort_unstable();
    times[times.len() / 2].as_micros()
}

fn aligned_error(found: Quad, expected: Quad) -> f32 {
    let expected_points = expected.points();
    let found_points = found.points();
    (0..4)
        .map(|shift| {
            (0..4)
                .map(|index| found_points[index].distance(expected_points[(index + shift) % 4]))
                .sum::<f32>()
                / 4.0
        })
        .fold(f32::INFINITY, f32::min)
}

fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    println!(
        "case,detector,detected,confidence,mean_corner_error_px,latency_median_us,correct,status"
    );
    let mut aggregate = [(0usize, 0usize, 0usize); 3]; // correct, eligible, false positives
    for case in cases() {
        for (detector_index, detector) in ["contour", "hough", "hybrid"].into_iter().enumerate() {
            let (_, _, warmup_panicked) = run(detector, &case.image);
            let mut times = Vec::with_capacity(RUNS);
            let mut result = None;
            let mut panicked = warmup_panicked;
            for _ in 0..RUNS {
                let (detection, elapsed, did_panic) = run(detector, &case.image);
                result = detection;
                panicked |= did_panic;
                times.push(elapsed);
            }
            let (confidence, error, correct, detected) = match (case.expected, result) {
                (Expected::Quad(expected), Some(found)) => {
                    let error = aligned_error(found.quad, expected);
                    let diagonal =
                        ((case.image.width().pow(2) + case.image.height().pow(2)) as f32).sqrt();
                    (found.confidence(), error, error <= diagonal * 0.03, true)
                }
                (Expected::Quad(_), None) => (0.0, f32::NAN, false, false),
                (Expected::None, Some(found)) => (found.confidence(), f32::NAN, false, true),
                (Expected::None, None) => (0.0, f32::NAN, true, false),
                (Expected::Ambiguous, Some(found)) => (found.confidence(), f32::NAN, false, true),
                (Expected::Ambiguous, None) => (0.0, f32::NAN, false, false),
            };
            let correct = correct && !panicked;
            if !matches!(case.expected, Expected::Ambiguous) {
                aggregate[detector_index].1 += 1;
                if correct {
                    aggregate[detector_index].0 += 1;
                }
                if matches!(case.expected, Expected::None) && detected {
                    aggregate[detector_index].2 += 1;
                }
            }
            println!(
                "{},{},{},{:.4},{},{},{},{}",
                case.name,
                detector,
                detected,
                confidence,
                if error.is_nan() {
                    "".into()
                } else {
                    format!("{error:.3}")
                },
                median_micros(&mut times),
                correct,
                if panicked { "panic" } else { "ok" }
            );
        }
    }
    eprintln!("detector,accuracy,false_positive_rate");
    for (index, detector) in ["contour", "hough", "hybrid"].into_iter().enumerate() {
        let (correct, eligible, false_positives) = aggregate[index];
        eprintln!(
            "{detector},{:.3},{:.3}",
            correct as f32 / eligible as f32,
            false_positives as f32 / 2.0
        );
    }
}
