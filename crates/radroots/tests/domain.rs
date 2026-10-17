use radroots::{
    OperationId,
    domain::{AmountError, Decimal, OrderId},
};

#[test]
fn ordinary_facade_keeps_exact_values_and_distinct_operation_identity() {
    let a: Decimal = "0.1".parse().unwrap();
    let b: Decimal = "0.2".parse().unwrap();
    assert_eq!(a.checked_add(b).unwrap().to_string(), "0.3");
    assert_eq!(a.checked_sub(b), Err(AmountError::NegativeResult));
    assert!("1e3".parse::<Decimal>().is_err());
    let operation: OperationId = "0123456789abcdef".repeat(4).parse().unwrap();
    assert_eq!(
        serde_json::to_string(&operation).unwrap(),
        format!("\"{operation}\"")
    );
    let order: OrderId = operation.to_string().parse().unwrap();
    assert_eq!(order.to_string(), operation.to_string());
    assert!(
        operation
            .to_string()
            .to_uppercase()
            .parse::<OperationId>()
            .is_err()
    );
}
