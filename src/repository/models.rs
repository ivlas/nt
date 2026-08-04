use crate::note::{Date, NoteId, NoteKind, Priority, QualifiedCollection, Status, Timestamp};

#[derive(Clone, Debug)]
pub struct NoteMeta {
    pub id: NoteId,
    pub home_collection: QualifiedCollection,
    pub body: String,
    pub created: Timestamp,
    pub updated: Timestamp,
    pub title: String,
    pub kind: NoteKind,
    pub status: Option<Status>,
    pub priority: Option<Priority>,
    pub scheduled: Option<Date>,
    pub due: Option<Date>,
    pub closed: Option<Timestamp>,
    pub tags: Vec<String>,
    pub collections: Vec<QualifiedCollection>,
    pub links: Vec<NoteId>,
    pub sources: Vec<String>,
}

impl NoteMeta {
    pub fn new_note(
        id: NoteId,
        home_collection: QualifiedCollection,
        body: String,
        created: Timestamp,
        updated: Timestamp,
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
    pub scheduled: Option<Date>,
    pub due: Option<Date>,
    pub created: Timestamp,
    pub title: String,
}

#[derive(Clone, Debug)]
pub struct FindRow {
    pub id: NoteId,
    pub created: Timestamp,
    pub title: String,
    pub tags: Vec<String>,
}

#[derive(Debug)]
pub enum NoteChange {
    Kind(NoteKind),
    Status(Option<Status>),
    Priority(Option<Priority>),
    Scheduled(Option<Date>),
    Due(Option<Date>),
    Home(QualifiedCollection),
    Tag {
        add: bool,
        value: String,
    },
    Collection {
        add: bool,
        value: QualifiedCollection,
    },
    Link {
        add: bool,
        value: NoteId,
    },
    Source {
        add: bool,
        value: String,
    },
}
