// P3 canonical operation fingerprint.
//
// Builds the normalized JSONB document that is durably stored alongside an
// idempotency record. Two semantically identical requests must produce
// IDENTICAL fingerprints AFTER all server-side defaults (operation_status
// default to `pending`, canonical currency code, trimmed strings, default
// metadata `{}`) have been applied — otherwise we would 409 a legitimate
// retry. Conversely, two requests that differ in any economically-relevant
// field must produce DIFFERENT fingerprints so the conflict path can reject
// the second attempt.
//
// The shape is deliberately conservative: only fields the server actually
// persists (`NewPortfolioOperation`) are included, plus the request kind and
// the correction link when present. We do NOT include the access token, the
// Authorization header, server-generated timestamps, or any transient
// frontend state.

use crate::domain::portfolio_operation::NewPortfolioOperation;
use crate::repositories::portfolio_operation_idempotency::IdempotencyRequestKind;
use serde_json::{Map, Value, json};
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

/// Build the canonical fingerprint that uniquely identifies a normalized
/// request payload for `(id_user, idempotency_key)` comparison.
pub fn build_fingerprint(
    id_user: Uuid,
    request_kind: IdempotencyRequestKind,
    id_corrected_operation: Option<Uuid>,
    operation: &NewPortfolioOperation,
) -> Value {
    let mut map: Map<String, Value> = Map::new();

    map.insert("id_user".to_string(), json!(id_user));
    map.insert("request_kind".to_string(), json!(request_kind.as_str()));
    map.insert(
        "id_corrected_operation".to_string(),
        json!(id_corrected_operation),
    );
    map.insert("id_portfolio".to_string(), json!(operation.id_portfolio));
    map.insert("id_asset".to_string(), json!(operation.id_asset));
    map.insert(
        "id_related_asset".to_string(),
        json!(operation.id_related_asset),
    );
    map.insert(
        "operation_type".to_string(),
        json!(operation.operation_type.as_str()),
    );
    map.insert(
        "operation_status".to_string(),
        json!(operation.operation_status.as_str()),
    );
    map.insert(
        "executed_at".to_string(),
        json!(format_datetime(&operation.executed_at)),
    );
    map.insert(
        "effective_at".to_string(),
        match &operation.effective_at {
            Some(value) => json!(format_datetime(value)),
            None => Value::Null,
        },
    );
    map.insert("quantity".to_string(), json!(operation.quantity));
    map.insert(
        "related_quantity".to_string(),
        json!(operation.related_quantity),
    );
    map.insert("price_minor".to_string(), json!(operation.price_minor));
    map.insert(
        "gross_amount_minor".to_string(),
        json!(operation.gross_amount_minor),
    );
    map.insert("fees_minor".to_string(), json!(operation.fees_minor));
    map.insert("taxes_minor".to_string(), json!(operation.taxes_minor));
    map.insert(
        "cash_amount_minor".to_string(),
        json!(operation.cash_amount_minor),
    );
    map.insert("currency".to_string(), json!(operation.currency));
    map.insert(
        "fx_rate_to_portfolio".to_string(),
        json!(operation.fx_rate_to_portfolio),
    );
    map.insert(
        "external_provider".to_string(),
        json!(operation.external_provider),
    );
    map.insert(
        "external_reference".to_string(),
        json!(operation.external_reference),
    );
    map.insert("notes".to_string(), json!(operation.notes));
    map.insert("metadata".to_string(), operation.metadata.clone());

    Value::Object(map)
}

fn format_datetime(value: &time::OffsetDateTime) -> String {
    value
        .format(&Rfc3339)
        .expect("OffsetDateTime should always be RFC3339 serializable")
}
