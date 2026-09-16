//! Wire types for the System One API.

mod entry;
mod models;
mod question;
mod request;
mod response;

pub use entry::Entry;
pub use models::{ListModelsResponse, ModelCard};
pub use question::{MAX_CHOICE_OPTIONS, NoulCriteria, Question, Questions, validate_questions};
pub use request::SystemOneRequest;
pub use response::{
    Answer, ChoiceView, NoulView, ResponseMeta, ScoreView, SystemOneResponse, Usage,
};
