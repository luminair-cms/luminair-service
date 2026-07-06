use std::io::Write;

#[test]
fn load_documents_from_directory() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let file_path = dir.path().join("mytype.json");
    let mut file = std::fs::File::create(&file_path).expect("create file");

    let content = r#"
    {
      "type": "collection",
      "info": {
        "title": "My Type",
        "description": "desc",
        "singularName": "mytype",
        "pluralName": "mytypes"
      },
      "options": {
        "draftAndPublish": true,
        "localizations": ["en"]
      },
      "attributes": {
        "name": {
          "type": "text",
          "unique": true,
          "required": true,
          "constraints": []
        },
        "owner": {
          "relation": "belongsToOne",
          "target": "user"
        }
      }
    }
    "#;

    file.write_all(content.as_bytes()).expect("write");
    file.sync_all().expect("sync");

    // call public loader
    let registry = common::load_documents(dir.path().to_str().unwrap()).expect("load docs");
    // lookup by api id (plural for collection)
    let dt = registry
        .lookup(&common::DocumentTypeApiId::try_new("mytypes").unwrap())
        .expect("found");
    assert_eq!(dt.id.as_ref(), "mytype");
    assert!(dt.has_draft_and_publish());

    // tempdir is dropped and cleaned up automatically
}
