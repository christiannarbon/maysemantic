pub mod ast {
    #[path = "builder_test.rs"]
    pub mod builder_test;
    #[path = "node_test.rs"]
    pub mod node_test;
}

pub mod dialects {
    #[path = "bigquery_test.rs"]
    pub mod bigquery_test;
    #[path = "core_test.rs"]
    pub mod core_test;
    #[path = "postgres_test.rs"]
    pub mod postgres_test;
    #[path = "snowflake_test.rs"]
    pub mod snowflake_test;
}

pub mod graph {
    #[path = "engine_test.rs"]
    pub mod engine_test;
    #[path = "resolver_test.rs"]
    pub mod resolver_test;
}

pub mod models {
    #[path = "state_test.rs"]
    pub mod state_test;
}

pub mod compiler {
    #[path = "request_test.rs"]
    pub mod request_test;
}

