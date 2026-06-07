#[cfg(test)]
mod aggregation_tests {
    use may_core::ast::{ColumnIdent, Expr};
    use may_core::models::AggregationType;

    fn col() -> Expr {
        Expr::Column(ColumnIdent("col".to_string()))
    }

    #[test]
    fn test_sum_to_expr() {
        let expr = AggregationType::Sum.to_expr(col());
        match expr {
            Expr::Function { name, args } => {
                assert_eq!(name, "SUM");
                assert_eq!(args.len(), 1);
                assert_eq!(args[0], col());
            }
            _ => panic!("Expected Expr::Function"),
        }
    }

    #[test]
    fn test_count_to_expr() {
        let expr = AggregationType::Count.to_expr(col());
        match expr {
            Expr::Function { name, args } => {
                assert_eq!(name, "COUNT");
                assert_eq!(args.len(), 1);
                assert_eq!(args[0], col());
            }
            _ => panic!("Expected Expr::Function"),
        }
    }

    #[test]
    fn test_average_produces_avg() {
        let expr = AggregationType::Average.to_expr(col());
        match expr {
            Expr::Function { name, args } => {
                assert_eq!(name, "AVG");
                assert_eq!(args.len(), 1);
                assert_eq!(args[0], col());
            }
            _ => panic!("Expected Expr::Function"),
        }
    }

    #[test]
    fn test_min_to_expr() {
        let expr = AggregationType::Min.to_expr(col());
        match expr {
            Expr::Function { name, args } => {
                assert_eq!(name, "MIN");
                assert_eq!(args.len(), 1);
                assert_eq!(args[0], col());
            }
            _ => panic!("Expected Expr::Function"),
        }
    }

    #[test]
    fn test_max_to_expr() {
        let expr = AggregationType::Max.to_expr(col());
        match expr {
            Expr::Function { name, args } => {
                assert_eq!(name, "MAX");
                assert_eq!(args.len(), 1);
                assert_eq!(args[0], col());
            }
            _ => panic!("Expected Expr::Function"),
        }
    }
}
