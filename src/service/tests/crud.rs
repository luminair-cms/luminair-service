mod common;

use common::*;

// ---------------------------------------------------------------------------
// Tests — health check
// ---------------------------------------------------------------------------

#[tokio::test]
async fn health_check_returns_200_empty_body() -> anyhow::Result<()> {
    let (router, _c) = build_router().await?;

    let response = router
        .oneshot(Request::builder().uri("/health").body(Body::empty())?)
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 1000).await?;
    assert!(bytes.is_empty());
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests — CRUD
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_and_retrieve_draft_document() -> anyhow::Result<()> {
    let (router, _c) = build_router().await?;

    let loc = create_brand(&router, "brand-a", "Alpha Brand").await?;

    let (status, json) = get_json(&router, &format!("{loc}?status=draft")).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["uid"], "brand-a");
    assert_eq!(json["data"]["name"], "Alpha Brand");
    Ok(())
}

#[tokio::test]
async fn duplicate_unique_field_returns_409_problem_details() -> anyhow::Result<()> {
    let (router, _c) = build_router().await?;

    create_brand(&router, "dup-uid", "Original").await?;

    let (status, _, bytes) = post_json(
        &router,
        "/api/documents/brands",
        r#"{"data": {"uid": "dup-uid", "name": "Duplicate"}}"#,
    )
    .await?;
    let json: Value = serde_json::from_slice(&bytes)?;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json["type"], "/errors/conflict");
    assert_eq!(json["status"], 409);
    Ok(())
}

#[tokio::test]
async fn nonexistent_relation_target_returns_422_problem_details() -> anyhow::Result<()> {
    let (router, _c) = build_router().await?;

    let phantom_id = uuid::Uuid::new_v4().to_string();
    let body = format!(
        r#"{{"data": {{"idno": "9000000000001", "legal_entity": "Ghost LLC", "category": {{"connect": ["{phantom_id}"]}}}}}}"#
    );
    let (status, _, bytes) = post_json(&router, "/api/documents/partners", &body).await?;
    let json: Value = serde_json::from_slice(&bytes)?;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(json["type"], "/errors/unprocessable-entity");
    assert_eq!(json["status"], 422);
    assert!(
        json["detail"]
            .as_str()
            .unwrap()
            .contains("Relation constraint violation"),
        "unexpected detail: {}",
        json["detail"]
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests — pagination cap
// ---------------------------------------------------------------------------

#[tokio::test]
async fn page_size_is_capped_at_configured_maximum() -> anyhow::Result<()> {
    let (router, _c) = build_router().await?;

    let (status, json) =
        get_json(&router, "/api/documents/brands?pagination[pageSize]=999").await?;

    assert_eq!(status, StatusCode::OK);
    let meta = &json["meta"];
    let page_size = meta
        .get("pageSize")
        .or_else(|| meta.get("page_size"))
        .and_then(|v| v.as_u64())
        .expect("pageSize must be present in meta");
    assert_eq!(page_size, 100, "pageSize must be capped at 100");
    Ok(())
}
