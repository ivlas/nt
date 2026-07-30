# index

Owns the SQLite schema and repository snapshot used by commands. The database
stores logical vaults, vault-owned collections, canonical note bodies and
metadata, many-to-many memberships, tags, links, and sources. Foreign keys and
transactions enforce consistency, including home membership.
