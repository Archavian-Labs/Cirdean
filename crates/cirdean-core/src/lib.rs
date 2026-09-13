//! Core domain types for Cirdean's document detection pipeline.
//!
//! This crate intentionally starts dependency-light. Image backends and
//! detector implementations can evolve without leaking GUI or camera concerns
//! into the public detection model.

pub mod detection;
pub mod geometry;
pub mod quality;

pub use detection::{Detection, DetectionMetrics, DetectionSource, Detector};
pub use geometry::{Point, Quad};
pub use quality::{CaptureQuality, QualityGate};
