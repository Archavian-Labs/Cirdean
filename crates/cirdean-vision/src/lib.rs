//! Pure-Rust document vision for Cirdean.
//!
//! The fast contour path is informed by Hbot's pragmatic multi-view detector.
//! The Hough fallback is informed by Camscan's line/intersection graph approach.
//! Cirdean adds candidate scoring, bounded combinatorics and staged execution.

pub mod book;
pub mod contour;
pub mod hough;
pub mod hybrid;
pub mod preprocess;
pub mod scoring;

pub use book::{GutterEstimate, estimate_gutter};
pub use contour::{ContourConfig, ContourDetector};
pub use hough::{HoughConfig, HoughDetector};
pub use hybrid::{HybridConfig, HybridDetector};
