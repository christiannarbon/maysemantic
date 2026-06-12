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
    #[path = "aggregation_test.rs"]
    pub mod aggregation_test;
    #[path = "entity_type_test.rs"]
    pub mod entity_type_test;
    #[path = "rls_entity_test.rs"]
    pub mod rls_entity_test;
    #[path = "state_test.rs"]
    pub mod state_test;
}

pub mod compiler {
    #[path = "chasm_trap_test.rs"]
    pub mod chasm_trap_test;
    #[path = "fanout_test.rs"]
    pub mod fanout_test;
    #[path = "join_builder_test.rs"]
    pub mod join_builder_test;
    #[path = "lowering_test.rs"]
    pub mod lowering_test;
    #[path = "metric_resolver_test.rs"]
    pub mod metric_resolver_test;
    #[path = "request_parser_test.rs"]
    pub mod request_parser_test;
    #[path = "request_test.rs"]
    pub mod request_test;
    #[path = "rls_test.rs"]
    pub mod rls_test;
}
