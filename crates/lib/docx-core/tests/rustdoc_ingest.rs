use std::path::PathBuf;

use docx_core::control::{DocxControlPlane, RustdocIngestRequest};
use docx_core::parsers::{RustdocJsonParser, RustdocParseOptions, RustdocParseOutput};
use surrealdb::engine::local::{Db, Mem};
use surrealdb::Surreal;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join("docx_store.json")
}

fn load_fixture() -> String {
    let path = fixture_path();
    std::fs::read_to_string(&path)
        .unwrap_or_else(|err| {
            let path_display = path.display();
            panic!("failed to read rustdoc fixture at {path_display}: {err}")
        })
}

fn parse_fixture(project_id: &str, ingest_id: &str) -> RustdocParseOutput {
    let json = load_fixture();
    let options = RustdocParseOptions::new(project_id).with_ingest_id(ingest_id);
    RustdocJsonParser::parse(&json, &options)
        .unwrap_or_else(|err| panic!("failed to parse rustdoc fixture: {err}"))
}

async fn build_control_plane(db_name: &str) -> DocxControlPlane<Db> {
    let db = Surreal::new::<Mem>(())
        .await
        .expect("failed to create in-memory surrealdb instance");
    db.use_ns("docx")
        .use_db(db_name)
        .await
        .expect("failed to select surrealdb namespace/db");
    DocxControlPlane::new(db)
}

#[tokio::test]
async fn ingest_rustdoc_fixture_roundtrip() {
    let project_id = "docx-store";
    let ingest_id = "fixture";
    let json = load_fixture();
    let parsed = parse_fixture(project_id, ingest_id);

    assert!(!parsed.symbols.is_empty(), "fixture should contain symbols");
    let named_symbol = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name.is_some())
        .expect("fixture should contain a named symbol");

    let control = build_control_plane("fixture").await;
    let report = control
        .ingest_rustdoc_json(RustdocIngestRequest {
            project_id: project_id.to_string(),
            json: Some(json.clone()),
            json_path: None,
            ingest_id: Some(ingest_id.to_string()),
            source_path: Some("target/doc/docx_store.json".to_string()),
            source_modified_at: None,
            tool_version: Some("fixture".to_string()),
            source_hash: None,
        })
        .await
        .expect("ingest should succeed");

    assert_eq!(report.crate_name, parsed.crate_name);
    assert_eq!(report.symbol_count, parsed.symbols.len());
    assert_eq!(report.doc_block_count, parsed.doc_blocks.len());
    assert!(report.doc_source_id.is_some());

    let search_name = named_symbol
        .name
        .as_ref()
        .expect("named symbol should have name");
    let search_results = control
        .search_symbols(project_id, search_name, 10)
        .await
        .expect("symbol search should succeed");
    assert!(
        !search_results.is_empty(),
        "symbol search should return results"
    );

    let kinds = control
        .list_symbol_kinds(project_id)
        .await
        .expect("symbol kind listing should succeed");
    assert!(!kinds.is_empty(), "fixture should yield symbol kinds");

    let adjacency = control
        .get_symbol_adjacency(project_id, &named_symbol.symbol_key, 50)
        .await
        .expect("symbol adjacency lookup should succeed");
    assert!(adjacency.symbol.is_some(), "adjacency should include symbol");
    assert!(
        !adjacency.doc_blocks.is_empty(),
        "adjacency should include doc blocks"
    );
    if !adjacency.doc_blocks.is_empty() {
        assert!(
            !adjacency.doc_sources.is_empty(),
            "adjacency should include doc sources when ingest metadata exists"
        );
    }

    if let Some(block) = parsed
        .doc_blocks
        .iter()
        .find(|block| block.symbol_key.is_some())
    {
        let symbol_key = block
            .symbol_key
            .as_ref()
            .expect("doc block should carry symbol key");
        let blocks = control
            .list_doc_blocks(project_id, symbol_key, Some(ingest_id))
            .await
            .expect("doc block lookup should succeed");
        assert!(!blocks.is_empty(), "doc blocks should be stored for symbol");
    }
}