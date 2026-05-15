use maysemantic::ast::SqlNode;
use maysemantic::ast_builder::ASTBuilder;

#[test]
fn test_builder_constructs_valid_semantic_ast() {
    // Build the same AST as test_ast_semantic_model using our new API
    let select =
        ASTBuilder::build_semantic_select(&[("locations", "region")], &[("orders", "revenue")]);

    let group_by = ASTBuilder::build_semantic_group_by(&[("locations", "region")]);

    let query = ASTBuilder::build_semantic_timespine_query("day", select, Some(group_by));

    // Validate the structure
    match query {
        SqlNode::Query {
            select,
            from,
            group_by,
            ..
        } => {
            // Validate Select
            if let SqlNode::Select(projection) = *select {
                assert_eq!(projection.len(), 2);
                if let SqlNode::DimensionRef { entity, dimension } = &projection[0] {
                    assert_eq!(entity, "locations");
                    assert_eq!(dimension, "region");
                } else {
                    panic!("Expected DimensionRef node in projection");
                }
            } else {
                panic!("Expected Select node");
            }

            // Validate From
            if let SqlNode::From { source, .. } = *from {
                if let SqlNode::TimeSpine { granularity } = *source {
                    assert_eq!(granularity, "day");
                } else {
                    panic!("Expected TimeSpine node inside FROM source");
                }
            } else {
                panic!("Expected From node");
            }

            // Validate GroupBy
            let group_outer = *group_by.expect("Expected GROUP BY clause");
            if let SqlNode::GroupBy(cols) = group_outer {
                assert_eq!(cols.len(), 1);
            } else {
                panic!("Expected GroupBy node");
            }
        }
        _ => panic!("Expected Query node at root"),
    }
}
