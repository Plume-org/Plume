table! {
    instances (id) {
        id -> Int4,
        local_domain -> Varchar,
        public_domain -> Varchar,
        name -> Varchar,
        local -> Bool,
        blocked -> Bool,
    }
}
