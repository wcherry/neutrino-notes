diesel::table! {
    notes (file_id) {
        file_id -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    note_links (source_note_id, target_note_id) {
        source_note_id -> Text,
        target_note_id -> Text,
        created_at -> Timestamp,
    }
}
