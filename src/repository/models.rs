use crate::note::{NoteId, NoteKind, Priority, Status};

#[derive(Clone, Debug)]
pub struct NoteMeta {
    pub id: NoteId,
    pub home_collection: String,
    pub body: String,
    pub created: String,
    pub updated: String,
    pub title: String,
    pub kind: NoteKind,
    pub status: Option<Status>,
    pub priority: Option<Priority>,
    pub scheduled: Option<String>,
    pub due: Option<String>,
    pub closed: Option<String>,
    pub tags: Vec<String>,
    pub collections: Vec<String>,
    pub links: Vec<NoteId>,
    pub sources: Vec<String>,
}

impl NoteMeta {
    pub fn new_note(
        id: NoteId,
        home_collection: String,
        body: String,
        created: String,
        updated: String,
        title: String,
    ) -> Self {
        Self {
            id,
            home_collection: home_collection.clone(),
            body,
            created,
            updated,
            title,
            kind: NoteKind::Note,
            status: None,
            priority: None,
            scheduled: None,
            due: None,
            closed: None,
            tags: Vec::new(),
            collections: vec![home_collection],
            links: Vec::new(),
            sources: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AgendaNote {
    pub id: NoteId,
    pub priority: Option<Priority>,
    pub scheduled: Option<String>,
    pub due: Option<String>,
    pub created: String,
    pub title: String,
}

#[derive(Clone, Debug)]
pub struct FindRow {
    pub id: NoteId,
    pub created: String,
    pub title: String,
    pub tags: Vec<String>,
}

#[derive(Debug)]
pub enum NoteChange {
    Kind(NoteKind),
    Status(Option<Status>),
    Priority(Option<Priority>),
    Scheduled(Option<String>),
    Due(Option<String>),
    Home(String),
    Tag { add: bool, value: String },
    Collection { add: bool, value: String },
    Link { add: bool, value: NoteId },
    Source { add: bool, value: String },
}
