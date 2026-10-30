//! Core of revisor: snapshots, storage and comparison. Knows nothing about the GUI.

pub mod scan;

/// Version of revisor, shared by every front end.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_semver() {
        let parts: Vec<&str> = version().split('.').collect();
        assert_eq!(parts.len(), 3, "{}", version());
        assert!(
            parts.iter().all(|p| p.parse::<u32>().is_ok()),
            "{}",
            version()
        );
    }
}
