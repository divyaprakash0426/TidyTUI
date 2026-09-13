//! Filesystem capacity for the volume holding a path, used to express junk
//! relative to the disk rather than against an arbitrary absolute threshold.

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiskUsage {
    pub total: u64,
    pub used: u64,
}

impl DiskUsage {
    pub fn used_fraction(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.used as f64 / self.total as f64
        }
    }

    /// `bytes` as a fraction of total capacity.
    pub fn fraction_of_total(&self, bytes: u64) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            bytes as f64 / self.total as f64
        }
    }
}

pub fn usage(path: &Path) -> Option<DiskUsage> {
    let stats = fs4::statvfs(path).ok()?;
    let total = stats.total_space();
    Some(DiskUsage {
        total,
        used: total.saturating_sub(stats.free_space()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractions_are_zero_safe() {
        let d = DiskUsage { total: 0, used: 0 };
        assert_eq!(d.used_fraction(), 0.0);
        assert_eq!(d.fraction_of_total(10), 0.0);
        let d = DiskUsage {
            total: 1000,
            used: 250,
        };
        assert!((d.used_fraction() - 0.25).abs() < 1e-9);
        assert!((d.fraction_of_total(50) - 0.05).abs() < 1e-9);
    }

    #[test]
    fn usage_of_root_is_plausible() {
        let d = usage(Path::new("/")).expect("statvfs on /");
        assert!(d.total > 0);
        assert!(d.used <= d.total);
    }
}
