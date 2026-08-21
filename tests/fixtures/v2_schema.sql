CREATE TABLE schema_version (
         singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
         version INTEGER NOT NULL CHECK (version = 2)
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
CREATE TABLE library_items (
         pk INTEGER PRIMARY KEY,
         id TEXT NOT NULL UNIQUE,
         source TEXT NOT NULL UNIQUE,
         title TEXT NOT NULL,
         created TEXT NOT NULL,
         updated TEXT NOT NULL,
         CHECK(length(id) = 36
               AND substr(id, 9, 1) = '-'
               AND substr(id, 14, 1) = '-'
               AND substr(id, 15, 1) = '7'
               AND substr(id, 19, 1) = '-'
               AND substr(id, 20, 1) IN ('8', '9', 'a', 'b')
               AND substr(id, 24, 1) = '-'
               AND length(replace(id, '-', '')) = 32
               AND replace(id, '-', '') NOT GLOB '*[^0-9a-f]*'),
         CHECK(length(source) > 0),
         CHECK(length(title) > 0),
         CHECK(created GLOB
               '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'),
         CHECK(updated GLOB
               '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z')
     );
CREATE TABLE library_captures (
         pk INTEGER PRIMARY KEY,
         item_pk INTEGER NOT NULL REFERENCES library_items(pk) ON DELETE CASCADE,
         captured TEXT NOT NULL,
         content TEXT NOT NULL,
         content_hash TEXT NOT NULL,
         UNIQUE(item_pk, content_hash),
         CHECK(captured GLOB
               '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'),
         CHECK(length(content) > 0),
         CHECK(length(content_hash) = 64
               AND content_hash NOT GLOB '*[^0-9a-f]*')
     );
CREATE TABLE library_summaries (
         capture_pk INTEGER PRIMARY KEY REFERENCES library_captures(pk) ON DELETE CASCADE,
         summary TEXT NOT NULL,
         generator TEXT NOT NULL,
         version TEXT NOT NULL,
         created TEXT NOT NULL,
         CHECK(length(summary) > 0),
         CHECK(length(generator) > 0),
         CHECK(length(version) > 0),
         CHECK(created GLOB
               '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z')
     );
CREATE VIRTUAL TABLE library_capture_fts USING fts5(
         content,
         content = 'library_captures',
         content_rowid = 'pk',
         tokenize = 'unicode61 remove_diacritics 2'
     );
CREATE TRIGGER library_captures_fts_insert AFTER INSERT ON library_captures BEGIN
         INSERT INTO library_capture_fts(rowid, content) VALUES (new.pk, new.content);
     END;
CREATE TRIGGER library_captures_fts_delete BEFORE DELETE ON library_captures BEGIN
         INSERT INTO library_capture_fts(library_capture_fts, rowid, content)
             VALUES ('delete', old.pk, old.content);
     END;
CREATE TRIGGER library_captures_immutable_update BEFORE UPDATE ON library_captures BEGIN
         SELECT RAISE(ABORT, 'library captures are immutable');
     END;
CREATE TRIGGER library_captures_immutable_delete BEFORE DELETE ON library_captures
     WHEN EXISTS (SELECT 1 FROM library_items WHERE pk = old.item_pk) BEGIN
         SELECT RAISE(ABORT, 'library captures are immutable');
     END;
CREATE INDEX library_items_updated_idx ON library_items(updated DESC, id DESC);
CREATE INDEX library_captures_latest_idx
         ON library_captures(item_pk, captured DESC, pk DESC);
CREATE TABLE note_library_refs (
         note_pk INTEGER NOT NULL REFERENCES notes(pk) ON DELETE CASCADE,
         library_item_pk INTEGER NOT NULL REFERENCES library_items(pk) ON DELETE CASCADE,
         PRIMARY KEY(note_pk, library_item_pk)
     );
CREATE INDEX note_library_refs_library_idx
         ON note_library_refs(library_item_pk, note_pk);
INSERT INTO schema_version(singleton, version) VALUES (1, 2);
PRAGMA application_id = 1314147924;
