#[derive(Clone, Debug)]
pub struct NoteMeta {
    pub id: String,
    pub home_collection: String,
    pub body: String,
    pub created: String,
    pub updated: String,
    pub title: String,
    pub kind: String,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub scheduled: Option<String>,
    pub due: Option<String>,
    pub closed: Option<String>,
    pub tags: Vec<String>,
    pub collections: Vec<String>,
    pub links: Vec<String>,
    pub sources: Vec<String>,
}

impl NoteMeta {
    pub fn new_note(
        id: String,
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
            kind: "note".to_string(),
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
    pub id: String,
    pub priority: Option<String>,
    pub scheduled: Option<String>,
    pub due: Option<String>,
    pub created: String,
    pub title: String,
}

#[derive(Clone, Debug)]
pub struct FindRow {
    pub id: String,
    pub created: String,
    pub title: String,
    pub tags: Vec<String>,
}

#[derive(Debug)]
pub enum NoteChange {
    Kind(String),
    Status(Option<String>),
    Priority(Option<String>),
    Scheduled(Option<String>),
    Due(Option<String>),
    Home(String),
    Tag { add: bool, value: String },
    Collection { add: bool, value: String },
    Link { add: bool, value: String },
    Source { add: bool, value: String },
}
