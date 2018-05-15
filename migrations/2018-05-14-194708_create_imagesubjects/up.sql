-- Your SQL goes here
CREATE TABLE image_subjects (
  id          SERIAL PRIMARY KEY, 
  image_id    SERIAL references images(id),
  subject_id  SERIAL references subjects(id)
)
