CREATE TABLE book_favorites (
    book_id          TEXT PRIMARY KEY REFERENCES books (id) ON DELETE CASCADE,
    owner_account_id TEXT NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    created_at       TEXT NOT NULL
);

CREATE INDEX idx_book_favorites_owner ON book_favorites (owner_account_id);
