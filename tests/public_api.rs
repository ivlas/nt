use nt::{
    AddOrRemove, App, Cli, CollectionPath, Command, Filter, InitOutcome, Input, NewNote, Note,
    NoteId, NoteQuery, NoteSummary, NtError, OpenMode, Repository, Result, Tag, Timestamp, run,
    timestamp_now,
};

#[test]
fn root_public_reexports_remain_available() {
    type_name::<AddOrRemove<Tag>>();
    type_name::<App<'static>>();
    type_name::<Cli>();
    type_name::<CollectionPath>();
    type_name::<Command>();
    type_name::<Filter>();
    type_name::<InitOutcome>();
    type_name::<Input<'static>>();
    type_name::<NewNote>();
    type_name::<Note>();
    type_name::<NoteId>();
    type_name::<NoteQuery>();
    type_name::<NoteSummary>();
    type_name::<NtError>();
    type_name::<OpenMode>();
    type_name::<Repository>();
    type_name::<Result<()>>();
    type_name::<Tag>();
    type_name::<Timestamp>();

    let _run = run;
    let _timestamp_now: fn() -> Result<Timestamp> = timestamp_now;
}

fn type_name<T>() -> &'static str {
    std::any::type_name::<T>()
}
