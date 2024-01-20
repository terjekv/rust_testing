// @generated automatically by Diesel CLI.

diesel::table! {
    nested_category (id) {
        id -> Int4,
        lft -> Int4,
        rgt -> Int4,
        #[max_length = 255]
        name -> Varchar,
    }
}
