CREATE TABLE post_translations (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    post_id INTEGER NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    source_lang VARCHAR NOT NULL,
    target_lang VARCHAR NOT NULL,
    title TEXT NOT NULL,
    subtitle TEXT NOT NULL,
    content TEXT NOT NULL,
    source TEXT NOT NULL,
    creation_date DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(post_id, target_lang)
);

CREATE INDEX post_translations_post_id ON post_translations (post_id);
CREATE INDEX post_translations_target_lang ON post_translations (target_lang);
