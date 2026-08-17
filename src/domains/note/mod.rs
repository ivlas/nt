mod domain;
pub(crate) mod query;
pub(crate) mod repository;
pub(crate) mod schema;

pub(crate) use domain::NoteRecord;
pub use domain::{CollectionPath, NewNote, Note, NoteId, Tag, Timestamp, timestamp_now};
pub use query::{Filter, NoteQuery};
pub use repository::{AddOrRemove, NoteSummary, Repository};
