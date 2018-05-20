table! {
    image_tags (id) {
        id -> Int4,
        image_id -> Int4,
        tag_id -> Int4,
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
    tags (id) {
        id -> Int4,
        content -> Array<Text>,
    }
}

joinable!(image_tags -> images (image_id));
joinable!(image_tags -> tags (tag_id));

allow_tables_to_appear_in_same_query!(
    image_tags,
    images,
    tags,
);
