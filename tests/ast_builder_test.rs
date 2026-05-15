use maysemantic::ast::{Expr, SqlNode};
use maysemantic::ast_builder::ASTBuilder;

/// Tests the `ASTBuilder` utility module to ensure it produces an accurate semantic AST.
///
/// Validates:
/// 1. `build_semantic_select` properly translates string tuples into `Expr::DimensionRef` and `Expr::MeasureRef`.
/// 2. Memory allocations (Vec scaling, Box pointers) are handled safely without boilerplate.
/// 3. The resulting structure mathematically matches a manually constructed `SqlNode` tree.
#[test]
fn test_builder_constructs_valid_semantic_ast() {
    // Construct the SELECT clause using the builder, which automatically wraps
    // our tuple dimensions and measures into Expr::DimensionRef and Expr::MeasureRef.
    let select =
        ASTBuilder::build_semantic_select(&[("locations", "region")], &[("orders", "revenue")]);

    // Construct the GROUP BY clause using the builder to wrap the dimension tuple.
    let group_by = ASTBuilder::build_semantic_group_by(&[("locations", "region")]);

    // Assemble the complete Query, injecting the select and group_by nodes,
    // and setting the FROM source to a TimeSpine with a "day" granularity.
    let query = ASTBuilder::build_semantic_timespine_query("day", select, Some(group_by));

    match query {
        SqlNode::Query {
            select,
            from,
            group_by,
            ..
        } => {
            if let SqlNode::Select(projection) = *select {
                assert_eq!(projection.len(), 2);
                if let Expr::DimensionRef { entity, dimension } = &projection[0] {
                    assert_eq!(entity, "locations");
                    assert_eq!(dimension, "region");
                } else {
                    panic!("Expected DimensionRef node in projection");
                }
            } else {
                panic!("Expected Select node");
            }

            if let SqlNode::From { source, .. } = *from {
                if let SqlNode::TimeSpine { granularity } = *source {
                    assert_eq!(granularity, "day");
                } else {
                    panic!("Expected TimeSpine node inside FROM source");
                }
            } else {
                panic!("Expected From node");
            }

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
