//! Core of revisor: snapshots, storage and comparison. Knows nothing about the GUI.

pub mod diff;
pub mod elevation;
pub mod exclude;
pub mod folders;
mod names;
pub mod profiles;
pub mod scan;
pub mod settings;
pub mod snapshot;
pub mod store;

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
