use serde_json::json;

use super::{QueryKind, validate_variable_matches};

#[test]
fn variable_matches_accepts_any_array_cardinality_but_requires_matches() {
    for value in [json!({ "matches": [] }), json!({ "matches": [{}] })] {
        validate_variable_matches(QueryKind::Definition, "variable server", &value)
            .expect("matches arrays of either cardinality should be accepted");
    }
    validate_variable_matches(QueryKind::Definition, "variable server", &json!({}))
        .expect_err("a missing matches field should be rejected");
}
