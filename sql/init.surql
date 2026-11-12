-- =============================================================================
-- SurrealDB Initialization Script for Hybrid Search (HNSW + BM25 + RRF) & GraphRAG
-- License: Apache 2.0 / MIT (Schema) | SurrealDB Core: BSL 1.1
-- =============================================================================

DEFINE NAMESPACE IF NOT EXISTS rag_namespace;
USE NS rag_namespace;
DEFINE DATABASE IF NOT EXISTS knowledge_base;
USE DB knowledge_base;

-- 1. Full-Text Analyzer (Supports German & English, tokenization + lowercase + ASCII folding)
DEFINE ANALYZER IF NOT EXISTS rag_analyzer
    TOKENIZERS class, punct
    FILTERS lowercase, ascii, snowball(german), snowball(english);

-- 2. Document Metadata Table (Parent Document)
DEFINE TABLE IF NOT EXISTS document SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS title      ON TABLE document TYPE string;
DEFINE FIELD IF NOT EXISTS source     ON TABLE document TYPE string;
DEFINE FIELD IF NOT EXISTS mime_type  ON TABLE document TYPE option<string>;
DEFINE FIELD IF NOT EXISTS metadata   ON TABLE document TYPE option<object> FLEXIBLE;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE document TYPE datetime DEFAULT time::now();

-- 3. Chunk Table (Text segments with dense vector + full-text content)
DEFINE TABLE IF NOT EXISTS chunk SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS doc_id      ON TABLE chunk TYPE string;
DEFINE FIELD IF NOT EXISTS doc_title   ON TABLE chunk TYPE string;
DEFINE FIELD IF NOT EXISTS chunk_index ON TABLE chunk TYPE int;
DEFINE FIELD IF NOT EXISTS content     ON TABLE chunk TYPE string;
DEFINE FIELD IF NOT EXISTS embedding   ON TABLE chunk TYPE array<float>;
DEFINE FIELD IF NOT EXISTS metadata    ON TABLE chunk TYPE option<object> FLEXIBLE;
DEFINE FIELD IF NOT EXISTS created_at  ON TABLE chunk TYPE datetime DEFAULT time::now();

-- 4. Indexes on the Chunk Table:
-- 4a) BM25 Full-Text Index for exact technical terms, model numbers, and keywords
DEFINE INDEX IF NOT EXISTS idx_chunk_bm25
    ON TABLE chunk
    FIELDS content
    FULLTEXT ANALYZER rag_analyzer BM25(1.2, 0.75) HIGHLIGHTS;

-- 4b) HNSW Vector Index (Default: 1024 dimensions for BAAI/bge-m3, Cosine Distance)
-- Note: setup.sh automatically updates DIMENSION if the 768d light preset is selected.
DEFINE INDEX IF NOT EXISTS idx_chunk_embedding
    ON TABLE chunk
    FIELDS embedding
    HNSW DIMENSION 1024 DIST COSINE EFC 150 M 16;

-- 5. Knowledge Graph Tables (Entities & Relations for GraphRAG)
DEFINE TABLE IF NOT EXISTS entity SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS name        ON TABLE entity TYPE string;
DEFINE FIELD IF NOT EXISTS entity_type ON TABLE entity TYPE string;
DEFINE FIELD IF NOT EXISTS description ON TABLE entity TYPE option<string>;
DEFINE INDEX IF NOT EXISTS idx_entity_name ON TABLE entity FIELDS name UNIQUE;

-- Graph edges: chunk -> mentions -> entity
DEFINE TABLE IF NOT EXISTS mentions TYPE RELATION IN chunk OUT entity;

-- Graph edges: entity -> relates_to -> entity
DEFINE TABLE IF NOT EXISTS relates_to TYPE RELATION IN entity OUT entity;
DEFINE FIELD IF NOT EXISTS relation_type ON TABLE relates_to TYPE string DEFAULT 'related_to';
DEFINE FIELD IF NOT EXISTS weight        ON TABLE relates_to TYPE float DEFAULT 1.0;

-- 6. Structured CRM / HR Employee Table (Relational + Graph Hybrid)
DEFINE TABLE IF NOT EXISTS employee_record SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS emp_id               ON TABLE employee_record TYPE string;
DEFINE FIELD IF NOT EXISTS name                 ON TABLE employee_record TYPE string;
DEFINE FIELD IF NOT EXISTS role                 ON TABLE employee_record TYPE string;
DEFINE FIELD IF NOT EXISTS department           ON TABLE employee_record TYPE string;
DEFINE FIELD IF NOT EXISTS skills               ON TABLE employee_record TYPE array<string>;
DEFINE FIELD IF NOT EXISTS english_level        ON TABLE employee_record TYPE string;
DEFINE FIELD IF NOT EXISTS certifications       ON TABLE employee_record TYPE array<string>;
DEFINE FIELD IF NOT EXISTS current_project      ON TABLE employee_record TYPE string;
DEFINE FIELD IF NOT EXISTS availability_percent ON TABLE employee_record TYPE int DEFAULT 100;
DEFINE FIELD IF NOT EXISTS hourly_rate          ON TABLE employee_record TYPE float DEFAULT 120.0;
DEFINE FIELD IF NOT EXISTS location             ON TABLE employee_record TYPE string DEFAULT 'Muenchen';
DEFINE FIELD IF NOT EXISTS updated_at           ON TABLE employee_record TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS idx_employee_emp_id  ON TABLE employee_record FIELDS emp_id UNIQUE;

