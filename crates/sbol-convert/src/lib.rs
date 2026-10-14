//! SBOL version conversion for the sbol-rs ecosystem.
//!
//! This crate converts between SBOL 2 and SBOL 3 at the RDF triple level.
//! [`upgrade`] converts a parsed [`sbol2::Document`] in memory, and
//! [`upgrade_from_sbol2`] and friends read SBOL 2 RDF text or files; both
//! build an [`sbol3::Document`]. [`downgrade`] takes an [`sbol3::Document`]
//! and emits the equivalent SBOL 2 RDF graph. Every conversion also returns
//! a report of any non-fatal issues encountered along the way.
#![forbid(unsafe_code)]
// Conversion errors carry the full context needed to diagnose a failed
// pipeline; boxing the large error arms would split that surface.
#![allow(clippy::result_large_err)]

mod downgrade;
mod sbol2_vocab;
mod upgrade;
mod uri;

pub use downgrade::{
    DowngradeCounts, DowngradeError, DowngradeOptions, DowngradeReport, DowngradeWarning,
    downgrade, downgrade_with, sbol3_to_sbol2,
};
pub use upgrade::{
    MapsToSide, NamespaceSource, UpgradeCounts, UpgradeError, UpgradeFromPathError, UpgradeOptions,
    UpgradeReport, UpgradeWarning, canonical_nt_line, parse_and_upgrade, sbol2_to_sbol3, upgrade,
    upgrade_from_sbol2, upgrade_from_sbol2_path, upgrade_from_sbol2_with, upgrade_with,
};
