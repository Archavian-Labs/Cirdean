use cirdean_core::{CaptureQuality, QualityGate};

fn main() {
    let gate = QualityGate::default();
    let quality = CaptureQuality {
        motion_stability: 0.0,
        corner_stability: 0.0,
        sharpness: 0.0,
        detection_confidence: 0.0,
        exposure_stability: 0.0,
    };

    println!(
        "Cirdean detection foundation initialized (capture_ready={}).",
        gate.accepts(quality)
    );
}
