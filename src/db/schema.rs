// @generated automatically by Diesel CLI.

diesel::table! {
    Unit (id) {
        id -> Text,
        imei -> Text,
        serialNum -> Text,
        productCode -> Nullable<Text>,
        createdAt -> Timestamp,
        updatedAt -> Timestamp,
    }
}

diesel::table! {
    _prisma_migrations (id) {
        #[max_length = 36]
        id -> Varchar,
        #[max_length = 64]
        checksum -> Varchar,
        finished_at -> Nullable<Timestamptz>,
        #[max_length = 255]
        migration_name -> Varchar,
        logs -> Nullable<Text>,
        rolled_back_at -> Nullable<Timestamptz>,
        started_at -> Timestamptz,
        applied_steps_count -> Int4,
    }
}

diesel::allow_tables_to_appear_in_same_query!(Unit, _prisma_migrations,);
