mod common;

use common::*;

// ---------------------------------------------------------------------------
// Tests — filter
// ---------------------------------------------------------------------------

#[tokio::test]
async fn filter_by_field_value() -> anyhow::Result<()> {
    let (router, _c) = build_router().await?;

    create_brand(&router, "fil-aaa", "Acme").await?;
    create_brand(&router, "fil-bbb", "Beta").await?;

    let (status, json) = get_json(
        &router,
        "/api/documents/brands?status=draft&filters[uid][$eq]=fil-aaa",
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    let items = json["data"].as_array().expect("data must be an array");
    assert_eq!(items.len(), 1, "filter should return exactly one brand");
    assert_eq!(items[0]["uid"], "fil-aaa");
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests — sort / order
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sort_documents_by_field_ascending() -> anyhow::Result<()> {
    let (router, _c) = build_router().await?;

    create_brand(&router, "srt-zzz", "Zebra").await?;
    create_brand(&router, "srt-aaa", "Apple").await?;
    create_brand(&router, "srt-mmm", "Mango").await?;

    let (status, json) = get_json(
        &router,
        "/api/documents/brands?status=draft&sort=uid:asc",
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    let items = json["data"].as_array().expect("data must be an array");
    assert!(items.len() >= 3, "expected at least 3 brands");

    let uids: Vec<&str> = items
        .iter()
        .map(|v| v["uid"].as_str().expect("uid field"))
        .collect();
    let mut sorted = uids.clone();
    sorted.sort();
    assert_eq!(uids, sorted, "brands should be in ascending uid order");
    Ok(())
}
