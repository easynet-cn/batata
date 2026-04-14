//! Vivaldi network coordinate algorithm.
//!
//! Implements the algorithm described in:
//!   Dabek, F., Cox, R., Kaashoek, F., & Morris, R. (2004).
//!   Vivaldi: A decentralized network coordinate system.
//!
//! The implementation follows Consul's `github.com/hashicorp/serf/coordinate`
//! package semantics so coordinates computed here can be compared to values
//! computed by Consul agents using the standard `lib/coordinate.Distance()`.
//!
//! # Algorithm overview
//!
//! Each node holds a coordinate:
//! - `vec`:        N-dimensional Euclidean position (default N=8)
//! - `height`:     non-Euclidean "height" accounting for tree-structured
//!                 bottlenecks like access-link latency
//! - `error`:      running estimate of prediction confidence (0..1)
//! - `adjustment`: gravity term averaging residual prediction error
//!
//! The **estimated RTT** between coordinates a and b is:
//!     d(a,b) = |a.vec - b.vec|_2 + a.height + b.height
//!
//! On each measurement of real RTT `rtt` to coordinate `other`, we:
//! 1. Compute relative error `es = |predicted - rtt| / rtt`
//! 2. Weight w = own.error / (own.error + other.error)
//! 3. Smooth confidence: own.error = es*CE*w + own.error*(1 - CE*w)
//! 4. Update position along the unit vector toward/away from `other`
//!    by `delta = CC * w * (rtt - predicted)`
//! 5. Update height: new_height = max(h_min, old_height + ...)
//! 6. Maintain an exponential moving average of residual error in `adjustment`
//!
//! Constants match Consul's defaults so cross-compatible values result.

use serde::{Deserialize, Serialize};

/// Dimensionality of the Euclidean portion of the coordinate.
pub const DIMENSIONALITY: usize = 8;

/// Minimum allowed height (prevents degenerate zero-height collapse).
pub const HEIGHT_MIN: f64 = 1.0e-5;

/// Initial "error" estimate for a new coordinate (1.5 = high uncertainty).
pub const VIVALDI_ERROR_MAX: f64 = 1.5;

/// Confidence constant (CE in the Vivaldi paper). Controls how quickly
/// the error estimate converges. Consul uses 0.25.
pub const VIVALDI_CE: f64 = 0.25;

/// Update constant (CC in the Vivaldi paper). Controls how large each
/// coordinate movement is relative to the prediction error. Consul uses 0.25.
pub const VIVALDI_CC: f64 = 0.25;

/// Window size for the adjustment moving average. Consul uses 20.
pub const ADJUSTMENT_WINDOW: usize = 20;

/// Maximum RTT sample to accept (protects against outliers). 10 seconds.
pub const MAX_RTT: std::time::Duration = std::time::Duration::from_secs(10);

/// Minimum RTT to consider valid (avoids div-by-zero near zero RTT).
pub const ZERO_RTT_THRESHOLD: f64 = 1.0e-6;

/// A Vivaldi coordinate in an N-dimensional space with a one-dimensional
/// height.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct VivaldiCoord {
    /// Euclidean portion — position in the coordinate space.
    pub vec: Vec<f64>,
    /// Error estimate (running confidence in this coordinate).
    pub error: f64,
    /// Adjustment term (EMA of residual error for asymmetric paths).
    pub adjustment: f64,
    /// Height term (models access-link latency).
    pub height: f64,
}

impl Default for VivaldiCoord {
    fn default() -> Self {
        Self::new(DIMENSIONALITY)
    }
}

impl VivaldiCoord {
    pub fn new(dim: usize) -> Self {
        Self {
            vec: vec![0.0; dim],
            error: VIVALDI_ERROR_MAX,
            adjustment: 0.0,
            height: HEIGHT_MIN,
        }
    }

    /// Check whether the coordinate contains only finite values.
    pub fn is_valid(&self) -> bool {
        self.vec.iter().all(|x| x.is_finite())
            && self.error.is_finite()
            && self.adjustment.is_finite()
            && self.height.is_finite()
    }

    /// Distance (predicted RTT) between two coordinates, in seconds.
    pub fn distance_to(&self, other: &VivaldiCoord) -> f64 {
        magnitude(&subtract(&self.vec, &other.vec)) + self.height + other.height
    }
}

/// Apply one Vivaldi update step.
///
/// `rtt` is the measured RTT to the remote node whose coordinate is `other`.
/// Returns the updated local coordinate.
///
/// The update is:
///   w = own.error / (own.error + other.error)
///   es = |predicted - rtt| / rtt
///   own.error = CE * es * w + own.error * (1 - CE * w)
///   own.vec = own.vec + delta * unit_vec(own.vec - other.vec)
///     where delta = CC * w * (rtt - predicted)
///   own.height = max(HEIGHT_MIN, (own.height + other.height) * (scale) + ...)
///   own.adjustment = EMA of residual over ADJUSTMENT_WINDOW samples
pub fn update(
    own: &VivaldiCoord,
    other: &VivaldiCoord,
    rtt_secs: f64,
    adjustment_samples: &mut Vec<f64>,
) -> VivaldiCoord {
    // Bail on non-finite or excessive RTT to avoid polluting the state
    if !rtt_secs.is_finite() || rtt_secs <= 0.0 || rtt_secs > MAX_RTT.as_secs_f64() {
        return own.clone();
    }
    if !own.is_valid() || !other.is_valid() {
        return own.clone();
    }

    let dist = own.distance_to(other).max(ZERO_RTT_THRESHOLD);

    // Confidence weight
    let total_err = (own.error + other.error).max(ZERO_RTT_THRESHOLD);
    let weight = own.error / total_err;

    // Relative prediction error
    let es = (dist - rtt_secs).abs() / rtt_secs;

    // Smooth confidence
    let new_error = ((VIVALDI_CE * weight) * es + own.error * (1.0 - VIVALDI_CE * weight))
        .min(VIVALDI_ERROR_MAX);

    // Force = CC * w * (rtt - predicted)
    let delta = VIVALDI_CC * weight * (rtt_secs - dist);

    // Unit vector from other toward own — we move own AWAY from other if
    // rtt > predicted (too close) and TOWARD other if rtt < predicted.
    let diff = subtract(&own.vec, &other.vec);
    let unit = unit_vector(&diff);

    // New euclidean position
    let new_vec: Vec<f64> = own
        .vec
        .iter()
        .zip(unit.iter())
        .map(|(v, u)| v + u * delta)
        .collect();

    // Update height. When the direction vector is zero we seed a small
    // perturbation so successive updates can make progress.
    let dir_magnitude = magnitude(&diff);
    let new_height = if dir_magnitude > ZERO_RTT_THRESHOLD {
        // Scale existing height by the same factor as the position update
        let new_h = (own.height + other.height) * delta / dist + own.height;
        new_h.max(HEIGHT_MIN)
    } else {
        own.height
    };

    // Maintain adjustment EMA
    adjustment_samples.push(rtt_secs - dist);
    while adjustment_samples.len() > ADJUSTMENT_WINDOW {
        adjustment_samples.remove(0);
    }
    let sum: f64 = adjustment_samples.iter().sum();
    let new_adjustment = sum / (2.0 * adjustment_samples.len() as f64);

    VivaldiCoord {
        vec: new_vec,
        error: new_error,
        adjustment: new_adjustment,
        height: new_height,
    }
}

// ---------- Vector math helpers ---------------------------------------------

fn subtract(a: &[f64], b: &[f64]) -> Vec<f64> {
    a.iter().zip(b.iter()).map(|(x, y)| x - y).collect()
}

fn magnitude(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

fn unit_vector(v: &[f64]) -> Vec<f64> {
    let mag = magnitude(v);
    if mag < ZERO_RTT_THRESHOLD {
        // Degenerate: return a small random direction. For determinism we
        // use a uniform tiny vector; real Consul uses randomness here.
        let small = 1.0 / (v.len() as f64).sqrt();
        return vec![small; v.len()];
    }
    v.iter().map(|x| x / mag).collect()
}

// ---------- Tests -----------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_coordinate_is_valid() {
        let c = VivaldiCoord::default();
        assert!(c.is_valid());
        assert_eq!(c.vec.len(), DIMENSIONALITY);
        assert_eq!(c.height, HEIGHT_MIN);
    }

    #[test]
    fn distance_between_origins_is_twice_height() {
        let a = VivaldiCoord::default();
        let b = VivaldiCoord::default();
        assert_eq!(a.distance_to(&b), 2.0 * HEIGHT_MIN);
    }

    #[test]
    fn single_update_reduces_error() {
        let a = VivaldiCoord::default();
        let mut b = VivaldiCoord::default();
        // Push b out along the x-axis so a has a predicted distance
        b.vec[0] = 0.1;

        let mut samples = Vec::new();
        let new_a = update(&a, &b, 0.1, &mut samples);

        // The error estimate should not increase (it's clamped at max)
        assert!(new_a.error <= a.error + 1.0e-9);
        // Coordinate should move
        assert_ne!(new_a.vec, a.vec);
    }

    #[test]
    fn update_converges_over_many_samples() {
        // Two nodes 100ms apart
        let target_rtt = 0.1;
        let mut a = VivaldiCoord::default();
        let b = VivaldiCoord {
            vec: {
                let mut v = vec![0.0; DIMENSIONALITY];
                v[0] = target_rtt;
                v
            },
            ..VivaldiCoord::default()
        };
        let mut samples = Vec::new();

        for _ in 0..200 {
            a = update(&a, &b, target_rtt, &mut samples);
        }

        let pred = a.distance_to(&b);
        // After 200 updates the predicted RTT should be reasonably close
        // to the measured RTT (within 50%)
        let relative_err = (pred - target_rtt).abs() / target_rtt;
        assert!(
            relative_err < 0.5,
            "converge failed: pred={}, target={}, err={}",
            pred,
            target_rtt,
            relative_err
        );
    }

    #[test]
    fn invalid_rtt_no_update() {
        let a = VivaldiCoord::default();
        let b = VivaldiCoord::default();
        let mut samples = Vec::new();

        // Negative, zero, infinite, NaN, and overly large RTTs should all be no-ops
        for bad in [-1.0, 0.0, f64::INFINITY, f64::NAN, 20.0] {
            let res = update(&a, &b, bad, &mut samples);
            assert_eq!(res.vec, a.vec);
            assert_eq!(res.error, a.error);
        }
    }

    #[test]
    fn error_is_bounded_below_vivaldi_max() {
        let mut a = VivaldiCoord::default();
        let b = VivaldiCoord {
            vec: {
                let mut v = vec![0.0; DIMENSIONALITY];
                v[0] = 0.05;
                v
            },
            ..VivaldiCoord::default()
        };
        let mut samples = Vec::new();
        for _ in 0..50 {
            a = update(&a, &b, 0.05, &mut samples);
            assert!(a.error <= VIVALDI_ERROR_MAX + 1.0e-9);
            assert!(a.error >= 0.0);
        }
    }

    #[test]
    fn height_never_goes_below_minimum() {
        let mut a = VivaldiCoord::default();
        let b = VivaldiCoord {
            vec: {
                let mut v = vec![0.0; DIMENSIONALITY];
                v[0] = 0.001;
                v
            },
            ..VivaldiCoord::default()
        };
        let mut samples = Vec::new();
        for _ in 0..100 {
            a = update(&a, &b, 0.001, &mut samples);
            assert!(a.height >= HEIGHT_MIN);
        }
    }
}
