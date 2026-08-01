//! Typed request and response models grouped by registry capability.

pub mod collection;
pub mod design;
pub mod instance;
pub mod oauth;
pub mod submission;

pub use collection::{
    CollectionDescriptor, CollectionPrecondition, CollectionRdfFormat, CollectionWrite,
    PulledCollection,
};
pub use design::{PulledDesign, SbolVersion};
pub use instance::{InstanceInfo, InstancePolicies, MachineAccess};
pub use oauth::{AuthorizationServerMetadata, OAuthClientRegistration, OAuthTokenResponse};
pub use submission::{
    CollisionPolicy, SubmissionConsequence, SubmissionCreated, SubmissionFormat, SubmissionNotice,
    SubmissionNoticeLevel, SubmissionPreview, SubmissionRequest,
};
