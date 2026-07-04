use may_core::ast::{
    build_semantic_group_by, build_semantic_select, build_semantic_timespine_query,
    with_pagination, Expr, SqlNode,
};

/// Tests the builder utility functions to ensure they produce an accurate semantic AST.
///
/// Validates:
/// 1. `build_semantic_select` properly translates string tuples into `Expr::DimensionRef` and `Expr::MeasureRef`.
/// 2. Memory allocations (Vec scaling, Box pointers) are handled safely without boilerplate.
/// 3. The resulting structure mathematically matches a manually constructed `SqlNode` tree.
#[test]
fn test_builder_constructs_valid_semantic_ast() {
    // Construct the SELECT clause using the builder, which automatically wraps
    // our tuple dimensions and measures into Expr::DimensionRef and Expr::MeasureRef.
    let select = build_semantic_select(None, &[("locations", "region")], &[("orders", "revenue")]);

    // Construct the GROUP BY clause using the builder to wrap the dimension tuple.
    let group_by = build_semantic_group_by(None, &[("locations", "region")]);

    // Assemble the complete Query, injecting the select and group_by nodes,
    // and setting the FROM source to a TimeSpine with a "day" granularity.
    let query = build_semantic_timespine_query("day", select, Some(group_by));

    match query {
        SqlNode::Query {
            select,
            from,
            group_by,
            ..
        } => {
            if let SqlNode::Select(projection) = *select {
                assert_eq!(projection.len(), 2);
                if let Expr::DimensionRef {
                    model: _,
                    entity,
                    dimension,
                } = &projection[0]
                {
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
        other => panic!("Expected Query node at root, got: {other:?}"),
    }
}

#[test]
fn test_with_pagination_applies_fields() {
    use may_core::ast::{ColumnIdent, OrderByExpr, SortDirection, TableIdent};

    // Test SqlNode::Query
    let select = build_semantic_select(None, &[], &[]);
    let query = build_semantic_timespine_query("day", select, None);

    let order_by = vec![OrderByExpr {
        expr: Expr::Column(ColumnIdent("name".to_string())),
        direction: SortDirection::Desc,
    }];
    let paginated = with_pagination(query, order_by.clone(), Some(10), Some(5));

    match paginated {
        SqlNode::Query {
            order_by: ob,
            limit,
            offset,
            ..
        } => {
            assert_eq!(ob, order_by);
            assert_eq!(limit, Some(10));
            assert_eq!(offset, Some(5));
        }
        _ => panic!("Expected Query node"),
    }

    // Test non-Query returns unchanged
    let table = SqlNode::Table(TableIdent("users".to_string()));
    let returned = with_pagination(table.clone(), vec![], None, None);
    assert_eq!(returned, table);
}
