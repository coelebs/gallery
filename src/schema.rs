table! {
    image_subjects (id) {
        id -> Int4,
        image_id -> Int4,
        subject_id -> Int4,
    }
}

table! {
    images (id) {
        id -> Int4,
        path -> Varchar,
        rating -> Int4,
        last_modified -> Timestamp,
        thumb_path -> Varchar,
        datetime -> Timestamp,
    }
}

table! {
    subjects (id) {
        id -> Int4,
        family -> Varchar,
        person -> Varchar,
    }
}

joinable!(image_subjects -> images (image_id));
joinable!(image_subjects -> subjects (subject_id));

allow_tables_to_appear_in_same_query!(
    image_subjects,
    images,
    subjects,
);
