//! Typed client for the machine-facing API exposed by an SBOL registry.
//!
//! The crate deliberately contains no SBOL object model. It moves registry
//! representations and typed metadata over HTTP; callers such as `sbol-cli`
//! parse and validate the returned document with the `sbol` crate. Keeping that
//! boundary lets registries evolve independently while the existing SBOL API
//! remains the authority for document semantics.
#![forbid(unsafe_code)]

pub mod client;
pub mod error;
pub mod models;

mod response;
mod url;

pub use client::RegistryClient;
pub use error::RegistryError;
pub use models::{
    AuthorizationServerMetadata, CollectionDescriptor, CollectionPrecondition, CollectionRdfFormat,
    CollectionWrite, CollisionPolicy, InstanceInfo, InstancePolicies, MachineAccess,
    OAuthClientRegistration, OAuthTokenResponse, PulledCollection, PulledDesign, SbolVersion,
    SubmissionConsequence, SubmissionCreated, SubmissionFormat, SubmissionNotice,
    SubmissionNoticeLevel, SubmissionPreview, SubmissionRequest,
};
