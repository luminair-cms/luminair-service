mod common;

use common::*;

// ---------------------------------------------------------------------------
// Tests — populate (relation loading)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn populate_loads_related_documents() -> anyhow::Result<()> {
    let (router, _c) = build_router().await?;

    let cat_loc = create_partner_category(&router, "pop-retail", 1).await?;
    let cat_id = cat_loc
        .trim_start_matches("/api/documents/partner-categories/");

    let partner_loc =
        create_partner(&router, "5000000000001", "Populated Partner Ltd").await?;
    let partner_id = partner_loc.trim_start_matches("/api/documents/partners/");

    // Connect the category to the partner
    let (status, _) = put_json(
        &router,
        &format!("/api/documents/partners/{partner_id}"),
        &format!(r#"{{"data": {{"category": {{"connect": ["{cat_id}"]}}}}}}"#),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);

    // Fetch with ?populate=category
    let (status, json) = get_json(
        &router,
        &format!("/api/documents/partners/{partner_id}?status=draft&populate=category"),
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    let category = &json["data"]["category"];
    assert!(
        category.as_array().map(|a| !a.is_empty()).unwrap_or(false),
        "category relation should be populated, got: {category}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests — connect / disconnect
// ---------------------------------------------------------------------------

#[tokio::test]
async fn connect_and_disconnect_relation() -> anyhow::Result<()> {
    let (router, _c) = build_router().await?;

    let cat_loc = create_partner_category(&router, "con-retail", 2).await?;
    let cat_id = cat_loc
        .trim_start_matches("/api/documents/partner-categories/");

    let partner_loc =
        create_partner(&router, "6000000000001", "Connect Test Ltd").await?;
    let partner_id = partner_loc.trim_start_matches("/api/documents/partners/");

    // --- Connect ---
    let (status, _) = put_json(
        &router,
        &format!("/api/documents/partners/{partner_id}"),
        &format!(r#"{{"data": {{"category": {{"connect": ["{cat_id}"]}}}}}}"#),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "connect should return 200");

    let (_, json) = get_json(
        &router,
        &format!("/api/documents/partners/{partner_id}?status=draft&populate=category"),
    )
    .await?;
    assert!(
        json["data"]["category"]
            .as_array()
            .map(|a| !a.is_empty())
            .unwrap_or(false),
        "category should be connected"
    );

    // --- Disconnect ---
    let (status, _) = put_json(
        &router,
        &format!("/api/documents/partners/{partner_id}"),
        &format!(r#"{{"data": {{"category": {{"disconnect": ["{cat_id}"]}}}}}}"#),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "disconnect should return 200");

    let (_, json) = get_json(
        &router,
        &format!("/api/documents/partners/{partner_id}?status=draft&populate=category"),
    )
    .await?;
    assert!(
        json["data"]["category"]
            .as_array()
            .map(|a| a.is_empty())
            .unwrap_or(true),
        "category should be disconnected"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests — publish
// ---------------------------------------------------------------------------

#[tokio::test]
async fn publish_draft_makes_document_visible_as_published() -> anyhow::Result<()> {
    let (router, _c) = build_router().await?;

    let loc = create_brand(&router, "pub-brd", "Published Brand").await?;

    // Draft exists
    let (status, _) = get_json(&router, &format!("{loc}?status=draft")).await?;
    assert_eq!(status, StatusCode::OK, "draft must be accessible");

    // Not published yet
    let (status, _) = get_json(&router, &loc).await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "should not be published yet");

    // Publish
    let published = publish_document(&router, &loc).await?;
    assert_eq!(published["data"]["uid"], "pub-brd");

    // Now available as published (default status)
    let (status, json) = get_json(&router, &loc).await?;
    assert_eq!(status, StatusCode::OK, "must be accessible after publish");
    assert_eq!(json["data"]["uid"], "pub-brd");
    Ok(())
}

#[tokio::test]
async fn draft_copy_still_accessible_after_publish() -> anyhow::Result<()> {
    let (router, _c) = build_router().await?;

    let loc = create_brand(&router, "pub-drft", "Still Draft").await?;
    publish_document(&router, &loc).await?;

    // Draft copy must still exist
    let (status, _) = get_json(&router, &format!("{loc}?status=draft")).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "draft copy should still be accessible after publish"
    );

    // Published copy must also exist
    let (status, _) = get_json(&router, &loc).await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "published copy must be accessible"
    );
    Ok(())
}
