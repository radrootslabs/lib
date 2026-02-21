use radroots_types::types::{IError, IResult, IResultList, IResultPass};

#[test]
fn error_wrapper_from_and_new_paths_are_exercised() {
    let from_impl: IError<&str> = IError::from("boom");
    assert_eq!(from_impl.err, "boom");

    let via_new = IError::new("bad");
    assert_eq!(via_new.err, "bad");
}

#[test]
fn result_wrapper_new_path_is_exercised() {
    let out = IResult::new(42u32);
    assert_eq!(out.result, 42);
}

#[test]
fn result_list_helpers_cover_empty_and_non_empty_branches() {
    let empty = IResultList::<u32>::new(Vec::new());
    assert!(empty.is_empty());

    let non_empty = IResultList::new(vec![1u32, 2u32]);
    assert!(!non_empty.is_empty());
    assert_eq!(non_empty.results, vec![1u32, 2u32]);
}

#[test]
fn result_pass_status_label_covers_both_branches() {
    let pass = IResultPass::new(true);
    let fail = IResultPass::new(false);
    assert_eq!(pass.status_label(), "pass");
    assert_eq!(fail.status_label(), "fail");
}

#[test]
fn serde_shapes_for_types_are_stable() {
    let err = serde_json::to_value(IError::new("boom")).unwrap();
    assert_eq!(err, serde_json::json!({ "err": "boom" }));

    let out = serde_json::to_value(IResult::new(7u32)).unwrap();
    assert_eq!(out, serde_json::json!({ "result": 7 }));

    let list = serde_json::to_value(IResultList::new(vec!["a", "b"])).unwrap();
    assert_eq!(list, serde_json::json!({ "results": ["a", "b"] }));

    let pass = serde_json::to_value(IResultPass::new(true)).unwrap();
    assert_eq!(pass, serde_json::json!({ "pass": true }));
}
