//! Compile-time guardrails for Auspex's Omegon compatibility envelope.
//!
//! This is intentionally narrow. It does not try to prove runtime protocol
//! compatibility; it prevents dependency drift from silently moving Auspex onto
//! a newer local Omegon traits checkout than the declared compatibility manifest
//! says we have audited.

/// Highest Omegon release line audited by this Auspex build.
pub const MAXIMUM_TESTED_OMEGON_VERSION: &str = "0.25.6";

/// Exact local/source `omegon-traits` version this crate is expected to compile
/// against while Auspex declares `MAXIMUM_TESTED_OMEGON_VERSION` above.
pub const EXPECTED_OMEGON_TRAITS_VERSION: &str = "0.25.6";

/// Actual linked `omegon-traits` package version.
pub const LINKED_OMEGON_TRAITS_VERSION: &str = env!("AUSPEX_LINKED_OMEGON_TRAITS_VERSION");

/// Returns an error when the linked traits crate has drifted past the audited
/// version. Patch/minor bumps must update this module, `omegon-compat.toml`,
/// and the `[package.metadata.omegon]` maximum tested version together.
pub fn assert_omegon_traits_version_pinned() -> Result<(), String> {
    if LINKED_OMEGON_TRAITS_VERSION == EXPECTED_OMEGON_TRAITS_VERSION {
        Ok(())
    } else {
        Err(format!(
            "Auspex declares Omegon {MAXIMUM_TESTED_OMEGON_VERSION} as maximum tested, but links omegon-traits {LINKED_OMEGON_TRAITS_VERSION}; update the compatibility audit before building against a new Omegon traits release"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linked_omegon_traits_version_matches_audited_pin() {
        assert!(
            assert_omegon_traits_version_pinned().is_ok(),
            "linked omegon-traits version must match the audited Auspex compatibility pin"
        );
    }
}
