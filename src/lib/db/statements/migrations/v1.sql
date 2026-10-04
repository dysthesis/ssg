PRAGMA foreign_keys = ON;

CREATE TABLE node (
    id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL,
    args BLOB NOT NULL,

    implementation_hash BLOB NOT NULL
        CHECK (length(implementation_hash) = 32),

    output_hash BLOB NOT NULL
        CHECK (length(output_hash) = 32),

    UNIQUE (kind, args)
) STRICT;

CREATE TABLE dependency (
    parent INTEGER NOT NULL
        REFERENCES node(id) ON DELETE CASCADE,

    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),

    dep INTEGER NOT NULL
        REFERENCES node(id) ON DELETE RESTRICT,

    expected BLOB NOT NULL
        CHECK (length(expected) = 32),

    PRIMARY KEY (parent, ordinal),
    CHECK (parent <> dep)
) STRICT, WITHOUT ROWID;

PRAGMA user_version = 1;
