diesel::table! {
    notes (file_id) {
        file_id -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}
