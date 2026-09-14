//! Local camera-frame benchmark; decoding and overlay writing are outside timing.
//! Usage: cargo run --release -p cirdean-vision --example benchmark_camera -- frame.png ...
use cirdean_core::{Detection, Detector};
use cirdean_vision::{ContourDetector, HoughDetector, HybridDetector};
use image::{GrayImage, Rgb};
use imageproc::drawing::{draw_hollow_circle_mut, draw_line_segment_mut};
use std::{env, error::Error, path::Path, time::Instant};

fn detect(kind: &str, frame: &GrayImage) -> Option<Detection> {
    match kind {
        "contour" => ContourDetector::default().detect(frame).unwrap(),
        "hough" => HoughDetector::default().detect(frame).unwrap(),
        "hybrid" => HybridDetector::default().detect(frame).unwrap(),
        _ => unreachable!(),
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    println!(
        "frame,width,height,detector,found,source,confidence,edge_score,geometry_score,median_ms,min_ms,max_ms,x0,y0,x1,y1,x2,y2,x3,y3"
    );
    for file in env::args().skip(1) {
        let path = Path::new(&file);
        let original = image::open(path)?;
        let gray = original.to_luma8();
        let stem = path.file_stem().unwrap().to_string_lossy();
        for kind in ["contour", "hough", "hybrid"] {
            let mut detection = detect(kind, &gray); // warm up
            let mut times = Vec::new();
            for _ in 0..7 {
                let start = Instant::now();
                detection = detect(kind, &gray);
                times.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            times.sort_by(f64::total_cmp);
            print!(
                "{stem},{},{},{kind},{},",
                gray.width(),
                gray.height(),
                detection.is_some()
            );
            if let Some(found) = detection {
                print!(
                    "{:?},{:.5},{:.5},{:.5},{:.3},{:.3},{:.3}",
                    found.source,
                    found.confidence(),
                    found.metrics.edge_score,
                    found.metrics.geometry_score,
                    times[3],
                    times[0],
                    times[6]
                );
                let mut overlay = original.to_rgb8();
                let color = match kind {
                    "contour" => Rgb([0, 255, 0]),
                    "hough" => Rgb([255, 180, 0]),
                    _ => Rgb([0, 255, 255]),
                };
                let points = found.quad.points();
                for i in 0..4 {
                    let a = points[i];
                    let b = points[(i + 1) % 4];
                    print!(",{:.2},{:.2}", a.x, a.y);
                    for offset in -1..=1 {
                        draw_line_segment_mut(
                            &mut overlay,
                            (a.x + offset as f32, a.y),
                            (b.x + offset as f32, b.y),
                            color,
                        );
                    }
                    draw_hollow_circle_mut(
                        &mut overlay,
                        (a.x.round() as i32, a.y.round() as i32),
                        9,
                        color,
                    );
                }
                overlay.save(path.with_file_name(format!("{stem}-{kind}.png")))?;
            } else {
                print!(
                    "none,,,,{:.3},{:.3},{:.3},,,,,,,,",
                    times[3], times[0], times[6]
                );
            }
            println!();
        }
    }
    Ok(())
}
