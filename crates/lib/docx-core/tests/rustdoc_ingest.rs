use std::path::PathBuf;

use docx_core::control::data::SearchSymbolsAdvancedRequest;
use docx_core::control::{DocxControlPlane, RustdocIngestReport, RustdocIngestRequest};
use docx_core::parsers::{RustdocJsonParser, RustdocParseOptions, RustdocParseOutput};
use surrealdb::Surreal;
use surrealdb::engine::local::{Db, Mem};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join("docx_store.json")
}

fn load_fixture() -> String {
    let path = fixture_path();
    std::fs::read_to_string(&path).unwrap_or_else(|err| {
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

async fn ingest_fixture(
    db_name: &str,
    project_id: &str,
    ingest_id: &str,
) -> (
    DocxControlPlane<Db>,
    RustdocParseOutput,
    RustdocIngestReport,
) {
    let json = load_fixture();
    let parsed = parse_fixture(project_id, ingest_id);
    let control = build_control_plane(db_name).await;
    let report = control
        .ingest_rustdoc_json(RustdocIngestRequest {
            project_id: project_id.to_string(),
            json: Some(json),
            json_path: None,
            ingest_id: Some(ingest_id.to_string()),
            source_path: Some("target/doc/docx_store.json".to_string()),
            source_modified_at: None,
            tool_version: Some("fixture".to_string()),
            source_hash: None,
        })
        .await
        .expect("ingest should succeed");
    (control, parsed, report)
}

#[tokio::test]
async fn ingest_rustdoc_fixture_roundtrip() {
    let project_id = "docx-store";
    let ingest_id = "fixture";
    let (control, parsed, report) = ingest_fixture("fixture", project_id, ingest_id).await;

    assert!(!parsed.symbols.is_empty(), "fixture should contain symbols");
    let named_symbol = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name.is_some())
        .expect("fixture should contain a named symbol");

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
    assert!(
        adjacency.symbol.is_some(),
        "adjacency should include symbol"
    );
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
    assert_eq!(
        adjacency.doc_sources.len(),
        adjacency.hydration_summary.deduped_total,
        "hydration summary should reflect final deduped source count"
    );

    let block = parsed
        .doc_blocks
        .iter()
        .find(|item| item.symbol_key.is_some())
        .expect("fixture should include at least one symbol-attached doc block");
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

#[tokio::test]
async fn adjacency_hydrates_doc_sources_from_observed_in_edges() {
    let project_id = "docx-store";
    let ingest_id = "fixture";
    let (control, parsed, _) = ingest_fixture("fixture-observed", project_id, ingest_id).await;

    let symbol_keys_with_docs = parsed
        .doc_blocks
        .iter()
        .filter_map(|block| block.symbol_key.as_deref())
        .collect::<std::collections::HashSet<_>>();
    let symbol_without_docs = parsed
        .symbols
        .iter()
        .find(|symbol| !symbol_keys_with_docs.contains(symbol.symbol_key.as_str()))
        .expect("fixture should include at least one symbol without doc blocks");
    let observed_only_adjacency = control
        .get_symbol_adjacency(project_id, &symbol_without_docs.symbol_key, 50)
        .await
        .expect("adjacency lookup for observed-only symbol should succeed");
    assert!(
        observed_only_adjacency.doc_blocks.is_empty(),
        "fixture symbol selected for observed-only check should have zero doc blocks"
    );
    assert!(
        !observed_only_adjacency.doc_sources.is_empty(),
        "adjacency should hydrate doc sources from observed_in edges"
    );
    assert!(
        observed_only_adjacency.hydration_summary.from_observed_in > 0,
        "observed_in hydration should contribute doc sources"
    );
}

#[tokio::test]
async fn advanced_search_and_completeness_audit_work() {
    let project_id = "docx-store";
    let ingest_id = "fixture";
    let (control, parsed, _) = ingest_fixture("fixture-audit", project_id, ingest_id).await;
    let named_symbol = parsed
        .symbols
        .iter()
        .find(|symbol| symbol.name.is_some())
        .expect("fixture should contain a named symbol");

    let advanced_search = control
        .search_symbols_advanced(
            project_id,
            SearchSymbolsAdvancedRequest {
                symbol_key: Some(named_symbol.symbol_key.clone()),
                ..SearchSymbolsAdvancedRequest::default()
            },
            10,
        )
        .await
        .expect("advanced symbol search should succeed");
    assert_eq!(
        advanced_search.total_returned, 1,
        "exact symbol_key filter should return a single symbol"
    );
    assert_eq!(
        advanced_search.symbols[0].symbol_key, named_symbol.symbol_key,
        "exact symbol_key filter should return the expected symbol"
    );

    let completeness = control
        .audit_project_completeness(project_id)
        .await
        .expect("project completeness audit should succeed");
    assert_eq!(
        completeness.symbol_count,
        parsed.symbols.len(),
        "completeness audit should report ingested symbol count"
    );
    assert_eq!(
        completeness.doc_block_count,
        parsed.doc_blocks.len(),
        "completeness audit should report ingested doc block count"
    );
    assert_eq!(
        completeness.doc_source_count, 1,
        "single ingest fixture should create one doc source"
    );
    assert!(
        completeness.symbols_with_observed_in_count > 0,
        "completeness audit should report observed_in coverage"
    );
    assert!(
        completeness
            .relation_counts
            .get("observed_in")
            .copied()
            .unwrap_or_default()
            > 0,
        "relation coverage should include observed_in edges"
    );
}

#[tokio::test]
async fn get_symbol_is_project_scoped() {
    let project_id = "docx-store";
    let ingest_id = "fixture";
    let (control, parsed, _) = ingest_fixture("fixture-project-scope", project_id, ingest_id).await;
    let symbol = parsed
        .symbols
        .first()
        .expect("fixture should include at least one symbol");

    let cross_project_symbol = control
        .get_symbol("unrelated-project", &symbol.symbol_key)
        .await
        .expect("cross-project symbol lookup should not error");
    assert!(
        cross_project_symbol.is_none(),
        "symbol lookup should not leak across project boundaries"
    );

    let cross_project_adjacency = control
        .get_symbol_adjacency("unrelated-project", &symbol.symbol_key, 50)
        .await
        .expect("cross-project adjacency lookup should not error");
    assert!(
        cross_project_adjacency.symbol.is_none(),
        "adjacency lookup should return empty payload for wrong project"
    );
}
