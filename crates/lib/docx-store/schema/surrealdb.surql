-- Canonical schema for docx-store (one database per solution)

DEFINE TABLE project SCHEMAFULL;
DEFINE FIELD project_id ON TABLE project TYPE string;
DEFINE FIELD name ON TABLE project TYPE string;
DEFINE FIELD language ON TABLE project TYPE string;
DEFINE FIELD root_path ON TABLE project TYPE string;
DEFINE FIELD description ON TABLE project TYPE string;
DEFINE FIELD extra ON TABLE project TYPE object;
DEFINE INDEX project_project_id ON TABLE project COLUMNS project_id UNIQUE;

DEFINE TABLE ingest SCHEMAFULL;
DEFINE FIELD project_id ON TABLE ingest TYPE string;
DEFINE FIELD git_commit ON TABLE ingest TYPE string;
DEFINE FIELD git_branch ON TABLE ingest TYPE string;
DEFINE FIELD git_tag ON TABLE ingest TYPE string;
DEFINE FIELD project_version ON TABLE ingest TYPE string;
DEFINE FIELD source_modified_at ON TABLE ingest TYPE datetime;
DEFINE FIELD ingested_at ON TABLE ingest TYPE datetime;
DEFINE FIELD extra ON TABLE ingest TYPE object;
DEFINE INDEX ingest_project_id ON TABLE ingest COLUMNS project_id;
DEFINE INDEX ingest_version ON TABLE ingest COLUMNS project_id, git_commit, git_branch, git_tag, project_version, source_modified_at;

DEFINE TABLE doc_source SCHEMAFULL;
DEFINE FIELD project_id ON TABLE doc_source TYPE string;
DEFINE FIELD ingest_id ON TABLE doc_source TYPE string;
DEFINE FIELD language ON TABLE doc_source TYPE string;
DEFINE FIELD source_kind ON TABLE doc_source TYPE string;
DEFINE FIELD path ON TABLE doc_source TYPE string;
DEFINE FIELD tool_version ON TABLE doc_source TYPE string;
DEFINE FIELD hash ON TABLE doc_source TYPE string;
DEFINE FIELD source_modified_at ON TABLE doc_source TYPE datetime;
DEFINE FIELD extra ON TABLE doc_source TYPE object;
DEFINE INDEX doc_source_project_id ON TABLE doc_source COLUMNS project_id;
DEFINE INDEX doc_source_ingest_id ON TABLE doc_source COLUMNS ingest_id;
DEFINE INDEX doc_source_path ON TABLE doc_source COLUMNS project_id, path;

DEFINE TABLE symbol SCHEMAFULL;
DEFINE FIELD project_id ON TABLE symbol TYPE string;
DEFINE FIELD language ON TABLE symbol TYPE string;
DEFINE FIELD symbol_key ON TABLE symbol TYPE string;
DEFINE FIELD kind ON TABLE symbol TYPE string;
DEFINE FIELD name ON TABLE symbol TYPE string;
DEFINE FIELD qualified_name ON TABLE symbol TYPE string;
DEFINE FIELD display_name ON TABLE symbol TYPE string;
DEFINE FIELD signature ON TABLE symbol TYPE string;
DEFINE FIELD signature_hash ON TABLE symbol TYPE string;
DEFINE FIELD visibility ON TABLE symbol TYPE string;
DEFINE FIELD is_static ON TABLE symbol TYPE bool;
DEFINE FIELD is_async ON TABLE symbol TYPE bool;
DEFINE FIELD is_const ON TABLE symbol TYPE bool;
DEFINE FIELD is_deprecated ON TABLE symbol TYPE bool;
DEFINE FIELD since ON TABLE symbol TYPE string;
DEFINE FIELD stability ON TABLE symbol TYPE string;
DEFINE FIELD source_path ON TABLE symbol TYPE string;
DEFINE FIELD line ON TABLE symbol TYPE int;
DEFINE FIELD col ON TABLE symbol TYPE int;
DEFINE FIELD return_type ON TABLE symbol TYPE object;
DEFINE FIELD params ON TABLE symbol TYPE array;
DEFINE FIELD type_params ON TABLE symbol TYPE array;
DEFINE FIELD attributes ON TABLE symbol TYPE array;
DEFINE FIELD source_ids ON TABLE symbol TYPE array;
DEFINE FIELD doc_summary ON TABLE symbol TYPE string;
DEFINE FIELD extra ON TABLE symbol TYPE object;
DEFINE INDEX symbol_project_id ON TABLE symbol COLUMNS project_id;
DEFINE INDEX symbol_key_index ON TABLE symbol COLUMNS project_id, symbol_key, language UNIQUE;
DEFINE INDEX symbol_name_index ON TABLE symbol COLUMNS project_id, name;
DEFINE INDEX symbol_qualified_name_index ON TABLE symbol COLUMNS project_id, qualified_name;

DEFINE TABLE doc_block SCHEMAFULL;
DEFINE FIELD project_id ON TABLE doc_block TYPE string;
DEFINE FIELD ingest_id ON TABLE doc_block TYPE string;
DEFINE FIELD symbol_key ON TABLE doc_block TYPE string;
DEFINE FIELD language ON TABLE doc_block TYPE string;
DEFINE FIELD source_kind ON TABLE doc_block TYPE string;
DEFINE FIELD doc_hash ON TABLE doc_block TYPE string;
DEFINE FIELD summary ON TABLE doc_block TYPE string;
DEFINE FIELD remarks ON TABLE doc_block TYPE string;
DEFINE FIELD returns ON TABLE doc_block TYPE string;
DEFINE FIELD value ON TABLE doc_block TYPE string;
DEFINE FIELD params ON TABLE doc_block TYPE array;
DEFINE FIELD type_params ON TABLE doc_block TYPE array;
DEFINE FIELD exceptions ON TABLE doc_block TYPE array;
DEFINE FIELD examples ON TABLE doc_block TYPE array;
DEFINE FIELD notes ON TABLE doc_block TYPE array;
DEFINE FIELD warnings ON TABLE doc_block TYPE array;
DEFINE FIELD safety ON TABLE doc_block TYPE string;
DEFINE FIELD panics ON TABLE doc_block TYPE string;
DEFINE FIELD errors ON TABLE doc_block TYPE string;
DEFINE FIELD see_also ON TABLE doc_block TYPE array;
DEFINE FIELD deprecated ON TABLE doc_block TYPE string;
DEFINE FIELD inherit_doc ON TABLE doc_block TYPE object;
DEFINE FIELD sections ON TABLE doc_block TYPE array;
DEFINE FIELD raw ON TABLE doc_block TYPE string;
DEFINE FIELD extra ON TABLE doc_block TYPE object;
DEFINE INDEX doc_block_project_id ON TABLE doc_block COLUMNS project_id;
DEFINE INDEX doc_block_symbol_index ON TABLE doc_block COLUMNS project_id, symbol_key, ingest_id;
DEFINE INDEX doc_block_doc_hash_index ON TABLE doc_block COLUMNS project_id, symbol_key, doc_hash;

DEFINE TABLE doc_chunk SCHEMAFULL;
DEFINE FIELD project_id ON TABLE doc_chunk TYPE string;
DEFINE FIELD ingest_id ON TABLE doc_chunk TYPE string;
DEFINE FIELD symbol_key ON TABLE doc_chunk TYPE string;
DEFINE FIELD doc_block_id ON TABLE doc_chunk TYPE string;
DEFINE FIELD chunk_index ON TABLE doc_chunk TYPE int;
DEFINE FIELD text ON TABLE doc_chunk TYPE string;
DEFINE FIELD token_count ON TABLE doc_chunk TYPE int;
DEFINE FIELD embedding ON TABLE doc_chunk TYPE array;
DEFINE FIELD extra ON TABLE doc_chunk TYPE object;
DEFINE INDEX doc_chunk_symbol_index ON TABLE doc_chunk COLUMNS project_id, symbol_key, ingest_id;

DEFINE TABLE contains TYPE RELATION SCHEMAFULL;
DEFINE FIELD project_id ON TABLE contains TYPE string;
DEFINE FIELD ingest_id ON TABLE contains TYPE string;
DEFINE FIELD kind ON TABLE contains TYPE string;
DEFINE FIELD extra ON TABLE contains TYPE object;

DEFINE TABLE member_of TYPE RELATION SCHEMAFULL;
DEFINE FIELD project_id ON TABLE member_of TYPE string;
DEFINE FIELD ingest_id ON TABLE member_of TYPE string;
DEFINE FIELD kind ON TABLE member_of TYPE string;
DEFINE FIELD extra ON TABLE member_of TYPE object;

DEFINE TABLE documents TYPE RELATION SCHEMAFULL;
DEFINE FIELD project_id ON TABLE documents TYPE string;
DEFINE FIELD ingest_id ON TABLE documents TYPE string;
DEFINE FIELD kind ON TABLE documents TYPE string;
DEFINE FIELD extra ON TABLE documents TYPE object;

DEFINE TABLE references TYPE RELATION SCHEMAFULL;
DEFINE FIELD project_id ON TABLE references TYPE string;
DEFINE FIELD ingest_id ON TABLE references TYPE string;
DEFINE FIELD kind ON TABLE references TYPE string;
DEFINE FIELD extra ON TABLE references TYPE object;

DEFINE TABLE see_also TYPE RELATION SCHEMAFULL;
DEFINE FIELD project_id ON TABLE see_also TYPE string;
DEFINE FIELD ingest_id ON TABLE see_also TYPE string;
DEFINE FIELD kind ON TABLE see_also TYPE string;
DEFINE FIELD extra ON TABLE see_also TYPE object;

DEFINE TABLE inherits TYPE RELATION SCHEMAFULL;
DEFINE FIELD project_id ON TABLE inherits TYPE string;
DEFINE FIELD ingest_id ON TABLE inherits TYPE string;
DEFINE FIELD kind ON TABLE inherits TYPE string;
DEFINE FIELD extra ON TABLE inherits TYPE object;

DEFINE TABLE implements TYPE RELATION SCHEMAFULL;
DEFINE FIELD project_id ON TABLE implements TYPE string;
DEFINE FIELD ingest_id ON TABLE implements TYPE string;
DEFINE FIELD kind ON TABLE implements TYPE string;
DEFINE FIELD extra ON TABLE implements TYPE object;

DEFINE TABLE overload_of TYPE RELATION SCHEMAFULL;
DEFINE FIELD project_id ON TABLE overload_of TYPE string;
DEFINE FIELD ingest_id ON TABLE overload_of TYPE string;
DEFINE FIELD kind ON TABLE overload_of TYPE string;
DEFINE FIELD extra ON TABLE overload_of TYPE object;

DEFINE TABLE type_of TYPE RELATION SCHEMAFULL;
DEFINE FIELD project_id ON TABLE type_of TYPE string;
DEFINE FIELD ingest_id ON TABLE type_of TYPE string;
DEFINE FIELD kind ON TABLE type_of TYPE string;
DEFINE FIELD extra ON TABLE type_of TYPE object;

DEFINE TABLE returns TYPE RELATION SCHEMAFULL;
DEFINE FIELD project_id ON TABLE returns TYPE string;
DEFINE FIELD ingest_id ON TABLE returns TYPE string;
DEFINE FIELD kind ON TABLE returns TYPE string;
DEFINE FIELD extra ON TABLE returns TYPE object;

DEFINE TABLE param_type TYPE RELATION SCHEMAFULL;
DEFINE FIELD project_id ON TABLE param_type TYPE string;
DEFINE FIELD ingest_id ON TABLE param_type TYPE string;
DEFINE FIELD kind ON TABLE param_type TYPE string;
DEFINE FIELD extra ON TABLE param_type TYPE object;

DEFINE TABLE observed_in TYPE RELATION SCHEMAFULL;
DEFINE FIELD project_id ON TABLE observed_in TYPE string;
DEFINE FIELD ingest_id ON TABLE observed_in TYPE string;
DEFINE FIELD kind ON TABLE observed_in TYPE string;
DEFINE FIELD extra ON TABLE observed_in TYPE object;
