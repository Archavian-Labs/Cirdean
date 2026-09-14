//! Evaluate the Rust detectors against Camscan's pinned real-image fixtures.
//!
//! The fixture annotations are copied from Camscan commit
//! `8c06d742dc77ad2c6768fdc873a0755764f1bb16`, `tests/test_scanner.py`.
//! Camscan's test accepts each corresponding corner at <= 30 px; this
//! evaluator reports cyclic mean and maximum corner error after the best
//! orientation alignment, and applies the upstream per-corner limit to max.
//!
//! Usage: cargo run -p cirdean-vision --example evaluate_images -- [fixture-root]
//!
//! MIT License
//!
//! Copyright (c) 2023 Adam Suhren Gustafsson
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the "Software"), to deal
//! in the Software without restriction, including without limitation the rights
//! to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//! copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in all
//! copies or substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//! SOFTWARE.

use std::{env, panic::AssertUnwindSafe, path::Path, time::Instant};

use cirdean_core::{Detection, Detector, Point, Quad};
use cirdean_vision::{ContourDetector, HoughDetector, HybridDetector};
use image::GrayImage;

const CAMSCAN_SHA: &str = "8c06d742dc77ad2c6768fdc873a0755764f1bb16";
const MAX_ERROR_PX: f32 = 30.0;

// Expected order is Camscan's (TL, TR, BR, BL), in original image pixels.
const CASES: [(&str, [[f32; 2]; 4]); 5] = [
    (
        "IMG_1842.jpg",
        [[47.0, 84.0], [938.0, 84.0], [950.0, 681.0], [47.0, 685.0]],
    ),
    (
        "IMG_1843.jpg",
        [
            [306.0, 101.0],
            [725.0, 310.0],
            [453.0, 869.0],
            [31.0, 667.0],
        ],
    ),
    (
        "IMG_1844.jpg",
        [
            [363.0, 154.0],
            [712.0, 339.0],
            [405.0, 845.0],
            [26.0, 576.0],
        ],
    ),
    (
        "IMG_1845.jpg",
        [
            [370.0, 266.0],
            [697.0, 356.0],
            [567.0, 837.0],
            [175.0, 678.0],
        ],
    ),
    (
        "IMG_1846.jpg",
        [
            [285.0, 164.0],
            [842.0, 269.0],
            [806.0, 681.0],
            [136.0, 515.0],
        ],
    ),
];

fn expected_quad(points: [[f32; 2]; 4]) -> Quad {
    Quad::new(
        Point::new(points[0][0], points[0][1]),
        Point::new(points[1][0], points[1][1]),
        Point::new(points[2][0], points[2][1]),
        Point::new(points[3][0], points[3][1]),
    )
}

fn cyclic_errors(expected: Quad, actual: Quad) -> (f32, f32) {
    let expected = expected.points();
    let actual = actual.points();
    (0..4)
        .map(|shift| {
            let errors: Vec<f32> = expected
                .iter()
                .enumerate()
                .map(|(index, point)| point.distance(actual[(index + shift) % 4]))
                .collect();
            let mean = errors.iter().sum::<f32>() / 4.0;
            let max = errors.into_iter().fold(0.0, f32::max);
            (mean, max)
        })
        .min_by(|left, right| left.0.total_cmp(&right.0))
        .expect("there are four cyclic alignments")
}

enum Outcome {
    Detection(Detection),
    None,
    Panic,
}

fn print_result(name: &str, outcome: Outcome, elapsed_ms: f64, expected: Quad) {
    let (status, mean, max, pass, confidence) = match outcome {
        Outcome::Detection(detection) => {
            let (mean, max) = cyclic_errors(expected, detection.quad);
            (
                "detected",
                mean,
                max,
                max <= MAX_ERROR_PX,
                detection.confidence(),
            )
        }
        Outcome::None => ("none", f32::NAN, f32::NAN, false, f32::NAN),
        Outcome::Panic => ("panic", f32::NAN, f32::NAN, false, f32::NAN),
    };
    println!("{name},{status},{mean:.2},{max:.2},{pass},{confidence:.3},{elapsed_ms:.3}");
}

fn evaluate<D: Detector<GrayImage, Error = std::convert::Infallible>>(
    label: &str,
    detector: &mut D,
    name: &str,
    image: &GrayImage,
    expected: Quad,
) {
    let started = Instant::now();
    let outcome = match std::panic::catch_unwind(AssertUnwindSafe(|| detector.detect(image))) {
        Ok(Ok(Some(detection))) => Outcome::Detection(detection),
        Ok(Ok(None)) => Outcome::None,
        Ok(Err(error)) => panic!("infallible detector failed: {error:?}"),
        Err(_) => {
            eprintln!("detector_panic={label} image={name}");
            Outcome::Panic
        }
    };
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    print!("{label},");
    print_result(name, outcome, elapsed_ms, expected);
}

fn main() {
    let root = env::args()
        .nth(1)
        .unwrap_or_else(|| "target/camscan-fixtures".to_owned());
    let root = Path::new(&root);
    eprintln!("fixture_source=camscan commit={CAMSCAN_SHA}");
    println!(
        "detector,image,status,mean_corner_error_px,max_corner_error_px,pass_30px,confidence,elapsed_ms"
    );

    for (name, points) in CASES {
        let path = root.join("images").join(name);
        let image = image::open(&path)
            .unwrap_or_else(|error| panic!("cannot open {}: {error}", path.display()))
            .into_luma8();
        let expected = expected_quad(points);

        evaluate(
            "contour",
            &mut ContourDetector::default(),
            name,
            &image,
            expected,
        );
        evaluate(
            "hough",
            &mut HoughDetector::default(),
            name,
            &image,
            expected,
        );
        evaluate(
            "hybrid",
            &mut HybridDetector::default(),
            name,
            &image,
            expected,
        );
    }
}
