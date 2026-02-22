-- Initial schema migration for docx-store (one database per solution)

DEFINE TABLE IF NOT EXISTS project SCHEMAFULL;

DEFINE FIELD IF NOT EXISTS project_id ON TABLE project TYPE string;
DEFINE FIELD IF NOT EXISTS name ON TABLE project TYPE option<string>;
DEFINE FIELD IF NOT EXISTS language ON TABLE project TYPE option<string>;
DEFINE FIELD IF NOT EXISTS root_path ON TABLE project TYPE option<string>;
DEFINE FIELD IF NOT EXISTS description ON TABLE project TYPE option<string>;
DEFINE FIELD IF NOT EXISTS aliases ON TABLE project TYPE option<array>;
DEFINE FIELD IF NOT EXISTS search_text ON TABLE project TYPE option<string>;
DEFINE FIELD IF NOT EXISTS extra ON TABLE project TYPE option<object> FLEXIBLE;

DEFINE INDEX IF NOT EXISTS project_project_id ON TABLE project COLUMNS project_id UNIQUE;
DEFINE INDEX IF NOT EXISTS project_search_text ON TABLE project COLUMNS search_text;

-- ============================================================================

DEFINE TABLE IF NOT EXISTS ingest SCHEMAFULL;

DEFINE FIELD IF NOT EXISTS project_id ON TABLE ingest TYPE string;
DEFINE FIELD IF NOT EXISTS git_commit ON TABLE ingest TYPE option<string>;
DEFINE FIELD IF NOT EXISTS git_branch ON TABLE ingest TYPE option<string>;
DEFINE FIELD IF NOT EXISTS git_tag ON TABLE ingest TYPE option<string>;
DEFINE FIELD IF NOT EXISTS project_version ON TABLE ingest TYPE option<string>;
DEFINE FIELD IF NOT EXISTS source_modified_at ON TABLE ingest TYPE option<string>;
DEFINE FIELD IF NOT EXISTS ingested_at ON TABLE ingest TYPE option<string>;
DEFINE FIELD IF NOT EXISTS extra ON TABLE ingest TYPE option<object> FLEXIBLE;

DEFINE INDEX IF NOT EXISTS ingest_project_id ON TABLE ingest COLUMNS project_id;
DEFINE INDEX IF NOT EXISTS ingest_version ON TABLE ingest COLUMNS project_id, git_commit, git_branch, git_tag, project_version, source_modified_at;

-- ============================================================================

DEFINE TABLE IF NOT EXISTS doc_source SCHEMAFULL;

DEFINE FIELD IF NOT EXISTS project_id ON TABLE doc_source TYPE string;
DEFINE FIELD IF NOT EXISTS ingest_id ON TABLE doc_source TYPE option<string>;
DEFINE FIELD IF NOT EXISTS language ON TABLE doc_source TYPE option<string>;
DEFINE FIELD IF NOT EXISTS source_kind ON TABLE doc_source TYPE option<string>;
DEFINE FIELD IF NOT EXISTS path ON TABLE doc_source TYPE option<string>;
DEFINE FIELD IF NOT EXISTS tool_version ON TABLE doc_source TYPE option<string>;
DEFINE FIELD IF NOT EXISTS hash ON TABLE doc_source TYPE option<string>;
DEFINE FIELD IF NOT EXISTS source_modified_at ON TABLE doc_source TYPE option<string>;
DEFINE FIELD IF NOT EXISTS extra ON TABLE doc_source TYPE option<object> FLEXIBLE;

DEFINE INDEX IF NOT EXISTS doc_source_project_id ON TABLE doc_source COLUMNS project_id;
DEFINE INDEX IF NOT EXISTS doc_source_ingest_id ON TABLE doc_source COLUMNS ingest_id;
DEFINE INDEX IF NOT EXISTS doc_source_path ON TABLE doc_source COLUMNS project_id, path;

-- ============================================================================

DEFINE TABLE IF NOT EXISTS symbol SCHEMAFULL;

DEFINE FIELD IF NOT EXISTS project_id ON TABLE symbol TYPE string;
DEFINE FIELD IF NOT EXISTS language ON TABLE symbol TYPE option<string>;
DEFINE FIELD IF NOT EXISTS symbol_key ON TABLE symbol TYPE string;
DEFINE FIELD IF NOT EXISTS kind ON TABLE symbol TYPE option<string>;
DEFINE FIELD IF NOT EXISTS name ON TABLE symbol TYPE option<string>;
DEFINE FIELD IF NOT EXISTS qualified_name ON TABLE symbol TYPE option<string>;
DEFINE FIELD IF NOT EXISTS display_name ON TABLE symbol TYPE option<string>;
DEFINE FIELD IF NOT EXISTS signature ON TABLE symbol TYPE option<string>;
DEFINE FIELD IF NOT EXISTS signature_hash ON TABLE symbol TYPE option<string>;
DEFINE FIELD IF NOT EXISTS visibility ON TABLE symbol TYPE option<string>;
DEFINE FIELD IF NOT EXISTS is_static ON TABLE symbol TYPE option<bool>;
DEFINE FIELD IF NOT EXISTS is_async ON TABLE symbol TYPE option<bool>;
DEFINE FIELD IF NOT EXISTS is_const ON TABLE symbol TYPE option<bool>;
DEFINE FIELD IF NOT EXISTS is_deprecated ON TABLE symbol TYPE option<bool>;
DEFINE FIELD IF NOT EXISTS since ON TABLE symbol TYPE option<string>;
DEFINE FIELD IF NOT EXISTS stability ON TABLE symbol TYPE option<string>;
DEFINE FIELD IF NOT EXISTS source_path ON TABLE symbol TYPE option<string>;
DEFINE FIELD IF NOT EXISTS line ON TABLE symbol TYPE option<int>;
DEFINE FIELD IF NOT EXISTS col ON TABLE symbol TYPE option<int>;
DEFINE FIELD IF NOT EXISTS return_type ON TABLE symbol TYPE option<object> FLEXIBLE;
DEFINE FIELD IF NOT EXISTS return_type.* ON TABLE symbol TYPE any;
DEFINE FIELD IF NOT EXISTS params ON TABLE symbol TYPE option<array<object>> FLEXIBLE;
DEFINE FIELD IF NOT EXISTS params[*].* ON TABLE symbol TYPE any;
DEFINE FIELD IF NOT EXISTS type_params ON TABLE symbol TYPE option<array<object>> FLEXIBLE;
DEFINE FIELD IF NOT EXISTS type_params[*].* ON TABLE symbol TYPE any;
DEFINE FIELD IF NOT EXISTS attributes ON TABLE symbol TYPE option<array<object>> FLEXIBLE;
DEFINE FIELD IF NOT EXISTS attributes[*].* ON TABLE symbol TYPE any;
DEFINE FIELD IF NOT EXISTS source_ids ON TABLE symbol TYPE option<array<object>> FLEXIBLE;
DEFINE FIELD IF NOT EXISTS source_ids[*].* ON TABLE symbol TYPE any;
DEFINE FIELD IF NOT EXISTS doc_summary ON TABLE symbol TYPE option<string>;
DEFINE FIELD IF NOT EXISTS extra ON TABLE symbol TYPE option<object> FLEXIBLE;
DEFINE FIELD IF NOT EXISTS extra.* ON TABLE symbol TYPE any;

DEFINE INDEX IF NOT EXISTS symbol_project_id ON TABLE symbol COLUMNS project_id;
DEFINE INDEX IF NOT EXISTS symbol_key_index ON TABLE symbol COLUMNS project_id, symbol_key, language UNIQUE;
DEFINE INDEX IF NOT EXISTS symbol_name_index ON TABLE symbol COLUMNS project_id, name;
DEFINE INDEX IF NOT EXISTS symbol_qualified_name_index ON TABLE symbol COLUMNS project_id, qualified_name;
DEFINE INDEX IF NOT EXISTS symbol_kind_index ON TABLE symbol COLUMNS project_id, kind;

-- ============================================================================

DEFINE TABLE IF NOT EXISTS doc_block SCHEMAFULL;

DEFINE FIELD IF NOT EXISTS project_id ON TABLE doc_block TYPE string;
DEFINE FIELD IF NOT EXISTS ingest_id ON TABLE doc_block TYPE option<string>;
DEFINE FIELD IF NOT EXISTS symbol_key ON TABLE doc_block TYPE option<string>;
DEFINE FIELD IF NOT EXISTS language ON TABLE doc_block TYPE option<string>;
DEFINE FIELD IF NOT EXISTS source_kind ON TABLE doc_block TYPE option<string>;
DEFINE FIELD IF NOT EXISTS doc_hash ON TABLE doc_block TYPE option<string>;
DEFINE FIELD IF NOT EXISTS summary ON TABLE doc_block TYPE option<string>;
DEFINE FIELD IF NOT EXISTS remarks ON TABLE doc_block TYPE option<string>;
DEFINE FIELD IF NOT EXISTS returns ON TABLE doc_block TYPE option<string>;
DEFINE FIELD IF NOT EXISTS value ON TABLE doc_block TYPE option<string>;
DEFINE FIELD IF NOT EXISTS params ON TABLE doc_block TYPE option<array<object>> FLEXIBLE;
DEFINE FIELD IF NOT EXISTS params[*].* ON TABLE doc_block TYPE any;
DEFINE FIELD IF NOT EXISTS type_params ON TABLE doc_block TYPE option<array<object>> FLEXIBLE;
DEFINE FIELD IF NOT EXISTS type_params[*].* ON TABLE doc_block TYPE any;
DEFINE FIELD IF NOT EXISTS exceptions ON TABLE doc_block TYPE option<array<object>> FLEXIBLE;
DEFINE FIELD IF NOT EXISTS exceptions[*].* ON TABLE doc_block TYPE any;
DEFINE FIELD IF NOT EXISTS examples ON TABLE doc_block TYPE option<array<object>> FLEXIBLE;
DEFINE FIELD IF NOT EXISTS examples[*].* ON TABLE doc_block TYPE any;
DEFINE FIELD IF NOT EXISTS notes ON TABLE doc_block TYPE option<array>;
DEFINE FIELD IF NOT EXISTS warnings ON TABLE doc_block TYPE option<array>;
DEFINE FIELD IF NOT EXISTS safety ON TABLE doc_block TYPE option<string>;
DEFINE FIELD IF NOT EXISTS panics ON TABLE doc_block TYPE option<string>;
DEFINE FIELD IF NOT EXISTS errors ON TABLE doc_block TYPE option<string>;
DEFINE FIELD IF NOT EXISTS see_also ON TABLE doc_block TYPE option<array<object>> FLEXIBLE;
DEFINE FIELD IF NOT EXISTS see_also[*].* ON TABLE doc_block TYPE any;
DEFINE FIELD IF NOT EXISTS deprecated ON TABLE doc_block TYPE option<string>;
DEFINE FIELD IF NOT EXISTS inherit_doc ON TABLE doc_block TYPE option<object> FLEXIBLE;
DEFINE FIELD IF NOT EXISTS inherit_doc.* ON TABLE doc_block TYPE any;
DEFINE FIELD IF NOT EXISTS sections ON TABLE doc_block TYPE option<array<object>> FLEXIBLE;
DEFINE FIELD IF NOT EXISTS sections[*].* ON TABLE doc_block TYPE any;
DEFINE FIELD IF NOT EXISTS raw ON TABLE doc_block TYPE option<string>;
DEFINE FIELD IF NOT EXISTS extra ON TABLE doc_block TYPE option<object> FLEXIBLE;
DEFINE FIELD IF NOT EXISTS extra.* ON TABLE doc_block TYPE any;

DEFINE INDEX IF NOT EXISTS doc_block_project_id ON TABLE doc_block COLUMNS project_id;
DEFINE INDEX IF NOT EXISTS doc_block_symbol_index ON TABLE doc_block COLUMNS project_id, symbol_key, ingest_id;
DEFINE INDEX IF NOT EXISTS doc_block_doc_hash_index ON TABLE doc_block COLUMNS project_id, symbol_key, doc_hash;

-- NOTE: Full-text search (SEARCH ANALYZER ... BM25) may not be supported by
-- all SurrealDB engines (e.g. kv-mem). The runtime applies this block
-- opportunistically and continues if the backend reports it as unsupported.
-- OPTIONAL_DOC_BLOCK_FTS_START
DEFINE ANALYZER IF NOT EXISTS docx_search TOKENIZERS blank,class FILTERS lowercase,snowball(english);
DEFINE INDEX IF NOT EXISTS doc_block_search_idx ON TABLE doc_block
    FIELDS summary, remarks, returns, errors, panics, safety
    SEARCH ANALYZER docx_search BM25;
-- OPTIONAL_DOC_BLOCK_FTS_END

-- ============================================================================

DEFINE TABLE IF NOT EXISTS doc_chunk SCHEMAFULL;

DEFINE FIELD IF NOT EXISTS project_id ON TABLE doc_chunk TYPE string;
DEFINE FIELD IF NOT EXISTS ingest_id ON TABLE doc_chunk TYPE option<string>;
DEFINE FIELD IF NOT EXISTS symbol_key ON TABLE doc_chunk TYPE option<string>;
DEFINE FIELD IF NOT EXISTS doc_block_id ON TABLE doc_chunk TYPE option<string>;
DEFINE FIELD IF NOT EXISTS chunk_index ON TABLE doc_chunk TYPE int;
DEFINE FIELD IF NOT EXISTS text ON TABLE doc_chunk TYPE string;
DEFINE FIELD IF NOT EXISTS token_count ON TABLE doc_chunk TYPE option<int>;
DEFINE FIELD IF NOT EXISTS embedding ON TABLE doc_chunk TYPE option<array>;
DEFINE FIELD IF NOT EXISTS extra ON TABLE doc_chunk TYPE option<object> FLEXIBLE;

DEFINE INDEX IF NOT EXISTS doc_chunk_symbol_index ON TABLE doc_chunk COLUMNS project_id, symbol_key, ingest_id;

-- ============================================================================

DEFINE TABLE IF NOT EXISTS contains TYPE RELATION IN symbol OUT symbol SCHEMAFULL;

DEFINE FIELD IF NOT EXISTS project_id ON TABLE contains TYPE string;
DEFINE FIELD IF NOT EXISTS ingest_id ON TABLE contains TYPE option<string>;
DEFINE FIELD IF NOT EXISTS kind ON TABLE contains TYPE option<string>;
DEFINE FIELD IF NOT EXISTS extra ON TABLE contains TYPE option<object> FLEXIBLE;

DEFINE INDEX IF NOT EXISTS contains_project_in ON TABLE contains COLUMNS project_id, in;
DEFINE INDEX IF NOT EXISTS contains_project_out ON TABLE contains COLUMNS project_id, out;

-- ============================================================================

DEFINE TABLE IF NOT EXISTS member_of TYPE RELATION IN symbol OUT symbol SCHEMAFULL;

DEFINE FIELD IF NOT EXISTS project_id ON TABLE member_of TYPE string;
DEFINE FIELD IF NOT EXISTS ingest_id ON TABLE member_of TYPE option<string>;
DEFINE FIELD IF NOT EXISTS kind ON TABLE member_of TYPE option<string>;
DEFINE FIELD IF NOT EXISTS extra ON TABLE member_of TYPE option<object> FLEXIBLE;

DEFINE INDEX IF NOT EXISTS member_of_project_in ON TABLE member_of COLUMNS project_id, in;
DEFINE INDEX IF NOT EXISTS member_of_project_out ON TABLE member_of COLUMNS project_id, out;

-- ============================================================================

DEFINE TABLE IF NOT EXISTS documents TYPE RELATION IN doc_block OUT symbol SCHEMAFULL;

DEFINE FIELD IF NOT EXISTS project_id ON TABLE documents TYPE string;
DEFINE FIELD IF NOT EXISTS ingest_id ON TABLE documents TYPE option<string>;
DEFINE FIELD IF NOT EXISTS kind ON TABLE documents TYPE option<string>;
DEFINE FIELD IF NOT EXISTS extra ON TABLE documents TYPE option<object> FLEXIBLE;

DEFINE INDEX IF NOT EXISTS documents_project_in ON TABLE documents COLUMNS project_id, in;
DEFINE INDEX IF NOT EXISTS documents_project_out ON TABLE documents COLUMNS project_id, out;

-- ============================================================================

DEFINE TABLE IF NOT EXISTS references TYPE RELATION IN symbol OUT symbol SCHEMAFULL;

DEFINE FIELD IF NOT EXISTS project_id ON TABLE references TYPE string;
DEFINE FIELD IF NOT EXISTS ingest_id ON TABLE references TYPE option<string>;
DEFINE FIELD IF NOT EXISTS kind ON TABLE references TYPE option<string>;
DEFINE FIELD IF NOT EXISTS extra ON TABLE references TYPE option<object> FLEXIBLE;

DEFINE INDEX IF NOT EXISTS references_project_in ON TABLE references COLUMNS project_id, in;
DEFINE INDEX IF NOT EXISTS references_project_out ON TABLE references COLUMNS project_id, out;

-- ============================================================================

DEFINE TABLE IF NOT EXISTS see_also TYPE RELATION IN symbol OUT symbol SCHEMAFULL;

DEFINE FIELD IF NOT EXISTS project_id ON TABLE see_also TYPE string;
DEFINE FIELD IF NOT EXISTS ingest_id ON TABLE see_also TYPE option<string>;
DEFINE FIELD IF NOT EXISTS kind ON TABLE see_also TYPE option<string>;
DEFINE FIELD IF NOT EXISTS extra ON TABLE see_also TYPE option<object> FLEXIBLE;

DEFINE INDEX IF NOT EXISTS see_also_project_in ON TABLE see_also COLUMNS project_id, in;
DEFINE INDEX IF NOT EXISTS see_also_project_out ON TABLE see_also COLUMNS project_id, out;

-- ============================================================================

DEFINE TABLE IF NOT EXISTS inherits TYPE RELATION IN symbol OUT symbol SCHEMAFULL;

DEFINE FIELD IF NOT EXISTS project_id ON TABLE inherits TYPE string;
DEFINE FIELD IF NOT EXISTS ingest_id ON TABLE inherits TYPE option<string>;
DEFINE FIELD IF NOT EXISTS kind ON TABLE inherits TYPE option<string>;
DEFINE FIELD IF NOT EXISTS extra ON TABLE inherits TYPE option<object> FLEXIBLE;

DEFINE INDEX IF NOT EXISTS inherits_project_in ON TABLE inherits COLUMNS project_id, in;
DEFINE INDEX IF NOT EXISTS inherits_project_out ON TABLE inherits COLUMNS project_id, out;

-- ============================================================================

DEFINE TABLE IF NOT EXISTS implements TYPE RELATION IN symbol OUT symbol SCHEMAFULL;

DEFINE FIELD IF NOT EXISTS project_id ON TABLE implements TYPE string;
DEFINE FIELD IF NOT EXISTS ingest_id ON TABLE implements TYPE option<string>;
DEFINE FIELD IF NOT EXISTS kind ON TABLE implements TYPE option<string>;
DEFINE FIELD IF NOT EXISTS extra ON TABLE implements TYPE option<object> FLEXIBLE;

DEFINE INDEX IF NOT EXISTS implements_project_in ON TABLE implements COLUMNS project_id, in;
DEFINE INDEX IF NOT EXISTS implements_project_out ON TABLE implements COLUMNS project_id, out;

-- ============================================================================

-- overload_of: reserved for C# method overloads; Rust does not populate this table.
DEFINE TABLE IF NOT EXISTS overload_of TYPE RELATION IN symbol OUT symbol SCHEMAFULL;

DEFINE FIELD IF NOT EXISTS project_id ON TABLE overload_of TYPE string;
DEFINE FIELD IF NOT EXISTS ingest_id ON TABLE overload_of TYPE option<string>;
DEFINE FIELD IF NOT EXISTS kind ON TABLE overload_of TYPE option<string>;
DEFINE FIELD IF NOT EXISTS extra ON TABLE overload_of TYPE option<object> FLEXIBLE;

DEFINE INDEX IF NOT EXISTS overload_of_project_in ON TABLE overload_of COLUMNS project_id, in;
DEFINE INDEX IF NOT EXISTS overload_of_project_out ON TABLE overload_of COLUMNS project_id, out;

-- ============================================================================

DEFINE TABLE IF NOT EXISTS type_of TYPE RELATION IN symbol OUT symbol SCHEMAFULL;

DEFINE FIELD IF NOT EXISTS project_id ON TABLE type_of TYPE string;
DEFINE FIELD IF NOT EXISTS ingest_id ON TABLE type_of TYPE option<string>;
DEFINE FIELD IF NOT EXISTS kind ON TABLE type_of TYPE option<string>;
DEFINE FIELD IF NOT EXISTS extra ON TABLE type_of TYPE option<object> FLEXIBLE;

DEFINE INDEX IF NOT EXISTS type_of_project_in ON TABLE type_of COLUMNS project_id, in;
DEFINE INDEX IF NOT EXISTS type_of_project_out ON TABLE type_of COLUMNS project_id, out;

-- ============================================================================

DEFINE TABLE IF NOT EXISTS returns TYPE RELATION IN symbol OUT symbol SCHEMAFULL;

DEFINE FIELD IF NOT EXISTS project_id ON TABLE returns TYPE string;
DEFINE FIELD IF NOT EXISTS ingest_id ON TABLE returns TYPE option<string>;
DEFINE FIELD IF NOT EXISTS kind ON TABLE returns TYPE option<string>;
DEFINE FIELD IF NOT EXISTS extra ON TABLE returns TYPE option<object> FLEXIBLE;

DEFINE INDEX IF NOT EXISTS returns_project_in ON TABLE returns COLUMNS project_id, in;
DEFINE INDEX IF NOT EXISTS returns_project_out ON TABLE returns COLUMNS project_id, out;

-- ============================================================================

DEFINE TABLE IF NOT EXISTS param_type TYPE RELATION IN symbol OUT symbol SCHEMAFULL;

DEFINE FIELD IF NOT EXISTS project_id ON TABLE param_type TYPE string;
DEFINE FIELD IF NOT EXISTS ingest_id ON TABLE param_type TYPE option<string>;
DEFINE FIELD IF NOT EXISTS kind ON TABLE param_type TYPE option<string>;
DEFINE FIELD IF NOT EXISTS extra ON TABLE param_type TYPE option<object> FLEXIBLE;

DEFINE INDEX IF NOT EXISTS param_type_project_in ON TABLE param_type COLUMNS project_id, in;
DEFINE INDEX IF NOT EXISTS param_type_project_out ON TABLE param_type COLUMNS project_id, out;

-- ============================================================================

DEFINE TABLE IF NOT EXISTS observed_in TYPE RELATION IN symbol OUT doc_source SCHEMAFULL;

DEFINE FIELD IF NOT EXISTS project_id ON TABLE observed_in TYPE string;
DEFINE FIELD IF NOT EXISTS ingest_id ON TABLE observed_in TYPE option<string>;
DEFINE FIELD IF NOT EXISTS kind ON TABLE observed_in TYPE option<string>;
DEFINE FIELD IF NOT EXISTS extra ON TABLE observed_in TYPE option<object> FLEXIBLE;

DEFINE INDEX IF NOT EXISTS observed_in_project_in ON TABLE observed_in COLUMNS project_id, in;
DEFINE INDEX IF NOT EXISTS observed_in_project_out ON TABLE observed_in COLUMNS project_id, out;
