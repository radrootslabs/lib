CREATE TABLE IF NOT EXISTS log_error (
    id CHAR(36) PRIMARY KEY NOT NULL UNIQUE CHECK(length(id) = 36),
    created_at DATETIME NOT NULL CHECK(length(created_at) = 24),
    updated_at DATETIME NOT NULL CHECK(length(updated_at) = 24),
    error TEXT NOT NULL,
    message TEXT NOT NULL,
    stack_trace TEXT,
    cause TEXT,
    app_system TEXT NOT NULL,
    app_version TEXT NOT NULL,
    nostr_pubkey TEXT NOT NULL,
    data TEXT
);