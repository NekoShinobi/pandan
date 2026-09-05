ALTER TABLE journal_nodes
ADD COLUMN emoji TEXT CHECK (
    emoji IS NULL
    OR (
        length(emoji) BETWEEN 1 AND 32
        AND length(CAST(emoji AS BLOB)) <= 128
    )
);
