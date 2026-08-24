CREATE TABLE schema_version (
         singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
         version INTEGER NOT NULL CHECK (version = 4)
     ) WITHOUT ROWID;
CREATE TABLE notes (
         pk INTEGER PRIMARY KEY,
         id TEXT NOT NULL UNIQUE,
         collection TEXT NOT NULL,
         body TEXT NOT NULL,
         title TEXT NOT NULL,
         created TEXT NOT NULL,
         updated TEXT NOT NULL,
         body_version INTEGER NOT NULL DEFAULT 1,
         CHECK(length(id) = 36
               AND substr(id, 9, 1) = '-'
               AND substr(id, 14, 1) = '-'
               AND substr(id, 15, 1) = '7'
               AND substr(id, 19, 1) = '-'
               AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')
               AND substr(id, 24, 1) = '-'
               AND length(replace(id, '-', '')) = 32
               AND replace(id, '-', '') NOT GLOB '*[^0-9a-f]*'),
         CHECK(length(collection) > 0
               AND collection NOT GLOB '*[^a-z0-9_/-]*'
               AND substr(collection, 1, 1) <> '/'
               AND substr(collection, -1, 1) <> '/'
               AND instr(collection, '//') = 0),
         CHECK(length(body) > 0),
         CHECK(length(title) > 0),
         CHECK(created GLOB
               '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'),
         CHECK(updated GLOB
               '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'),
         CHECK(body_version > 0)
     );
CREATE TABLE note_tags (
         note_pk INTEGER NOT NULL REFERENCES notes(pk) ON DELETE CASCADE,
         tag TEXT NOT NULL,
         PRIMARY KEY(note_pk, tag),
         CHECK(length(tag) > 0 AND tag NOT GLOB '*[^a-z0-9_-]*')
     );
CREATE TABLE note_links (
         note_pk INTEGER NOT NULL REFERENCES notes(pk) ON DELETE CASCADE,
         target_note_pk INTEGER NOT NULL REFERENCES notes(pk) ON DELETE CASCADE,
         PRIMARY KEY(note_pk, target_note_pk),
         CHECK(note_pk <> target_note_pk)
     );
CREATE VIRTUAL TABLE note_fts USING fts5(
         title,
         body,
         content = 'notes',
         content_rowid = 'pk',
         tokenize = 'unicode61 remove_diacritics 2'
     );
CREATE TRIGGER notes_fts_insert AFTER INSERT ON notes BEGIN
         INSERT INTO note_fts(rowid, title, body) VALUES (new.pk, new.title, new.body);
     END;
CREATE TRIGGER notes_fts_update AFTER UPDATE OF title, body ON notes BEGIN
         INSERT INTO note_fts(note_fts, rowid, title, body)
             VALUES ('delete', old.pk, old.title, old.body);
         INSERT INTO note_fts(rowid, title, body) VALUES (new.pk, new.title, new.body);
     END;
CREATE TRIGGER notes_fts_delete BEFORE DELETE ON notes BEGIN
         INSERT INTO note_fts(note_fts, rowid, title, body)
             VALUES ('delete', old.pk, old.title, old.body);
     END;
CREATE INDEX notes_created_idx ON notes(created DESC, id DESC);
CREATE INDEX notes_updated_idx ON notes(updated DESC, id DESC);
CREATE INDEX notes_collection_updated_idx
         ON notes(collection, updated DESC, id DESC);
CREATE INDEX note_tags_tag_note_idx ON note_tags(tag, note_pk);
CREATE INDEX note_links_target_idx ON note_links(target_note_pk);
INSERT INTO schema_version(singleton, version) VALUES (1, 4);
PRAGMA application_id = 1314147924;
