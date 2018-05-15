table! {
    image_subjects (id) {
        id -> Integer,
        image_id -> Integer,
        subject_id -> Integer,
    }
}

table! {
    images (id) {
        id -> Integer,
        path -> Text,
        rating -> Integer,
        last_modified -> Timestamp,
        thumb_path -> Text,
    }
}

table! {
    subjects (id) {
        id -> Integer,
        family -> Text,
        person -> Text,
    }
}

allow_tables_to_appear_in_same_query!(
    image_subjects,
    images,
    subjects,
);
