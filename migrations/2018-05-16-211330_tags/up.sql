-- Your SQL goes here
CREATE TABLE tags (
  id          SERIAL PRIMARY KEY,
  content     TEXT ARRAY NOT NULL
)
