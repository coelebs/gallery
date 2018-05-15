-- Your SQL goes here
CREATE TABLE images (
  id              SERIAL PRIMARY KEY,
  path            VARCHAR NOT NULL,
  rating          INTEGER NOT NULL,
  last_modified   TIMESTAMP NOT NULL,
  thumb_path      VARCHAR NOT NULL
)
