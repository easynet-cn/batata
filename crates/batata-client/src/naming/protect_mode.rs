//! Protect mode for service discovery
//!
//! Protects service availability when the number of healthy instances
//! drops below a threshold.

/// Protect mode configuration
#[derive(Debug, Clone)]
pub struct ProtectMode {
    protect_threshold: f32,
}

impl ProtectMode {
    /// Create with default threshold (0.8)
    pub fn new() -> Self {
        Self {
            protect_threshold: 0.8,
        }
    }

    /// Create with custom threshold
    pub fn with_threshold(protect_threshold: f32) -> Self {
        Self {
            protect_threshold,
        }
    }

    /// Get the protection threshold (0.0 - 1.0)
    pub fn protect_threshold(&self) -> f32 {
        self.protect_threshold
    }

    /// Set the protection threshold
    pub fn set_protect_threshold(&mut self, threshold: f32) {
        assert!((0.0..=1.0).contains(&threshold), "Threshold must be between 0.0 and 1.0");
        self.protect_threshold = threshold;
    }

    /// Check if protection should be enabled
    /// 
    /// Protection is enabled when the ratio of healthy instances
    /// to total instances falls below the threshold.
    pub fn should_protect(&self, healthy_count: usize, total_count: usize) -> bool {
        if total_count == 0 {
            return false;
        }

        let ratio = healthy_count as f32 / total_count as f32;
        ratio < self.protect_threshold
    }

    /// Check if we should return empty result (protected) or fallback
    pub fn is_protected(&self, healthy_count: usize, total_count: usize) -> bool {
        self.should_protect(healthy_count, total_count) && healthy_count > 0
    }
}

impl Default for ProtectMode {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_threshold() {
        let protect = ProtectMode::new();
        assert_eq!(protect.protect_threshold(), 0.8);
    }

    #[test]
    fn test_custom_threshold() {
        let protect = ProtectMode::with_threshold(0.6);
        assert_eq!(protect.protect_threshold(), 0.6);
    }

    #[test]
    fn test_should_protect() {
        let protect = ProtectMode::with_threshold(0.8);

        // 4/10 = 0.4 < 0.8, should protect
        assert!(protect.should_protect(4, 10));

        // 9/10 = 0.9 > 0.8, should not protect
        assert!(!protect.should_protect(9, 10));

        // 0/0, should not protect
        assert!(!protect.should_protect(0, 0));
    }

    #[test]
    fn test_is_protected() {
        let protect = ProtectMode::with_threshold(0.8);

        // 4/10 < threshold but has healthy instances
        assert!(protect.is_protected(4, 10));

        // 0/10, should not return protected (no fallback available)
        assert!(!protect.is_protected(0, 10));

        // 9/10 > threshold
        assert!(!protect.is_protected(9, 10));
    }

    #[test]
    #[should_panic]
    fn test_invalid_threshold() {
        ProtectMode::with_threshold(1.5);
    }
}
