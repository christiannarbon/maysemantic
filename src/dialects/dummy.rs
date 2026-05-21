pub(crate) mod test_support {
    use crate::dialects::SqlDialect;

    /// A minimal dialect for testing the default ANSI `SqlDialect` trait implementations.
    #[derive(Debug)]
    pub struct DummyDialect;

    impl SqlDialect for DummyDialect {}
}
