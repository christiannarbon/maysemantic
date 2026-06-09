use may_core::ast::{SqlNode, TableIdent};
use may_core::compiler::{ChasmTrapError, ChasmTrapHandler, PathClassification};

#[test]
fn test_inject_ctes_single_fact() {
    let query = SqlNode::Table(TableIdent("orders".to_string()));
    let classification = PathClassification::SingleFact;
    let result = ChasmTrapHandler::inject_ctes(query.clone(), &classification, "user_id");
    assert_eq!(result.unwrap(), query);
}

#[test]
fn test_inject_ctes_pure_dimension() {
    let query = SqlNode::Table(TableIdent("users".to_string()));
    let classification = PathClassification::PureDimension;
    let result = ChasmTrapHandler::inject_ctes(query.clone(), &classification, "user_id");
    assert_eq!(result.unwrap(), query);
}

#[test]
fn test_inject_ctes_empty_fact_tables() {
    let query = SqlNode::Table(TableIdent("orders".to_string()));
    let classification = PathClassification::MultiFactJoin {
        fact_tables: vec![],
    };
    let result = ChasmTrapHandler::inject_ctes(query, &classification, "user_id");
    assert_eq!(result.unwrap_err(), ChasmTrapError::EmptyFactTableList);
}
