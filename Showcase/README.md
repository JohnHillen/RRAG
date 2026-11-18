# 🎯 Showcases Overview (`./Showcase/`)

By default, running `./setup.sh` starts the system with **no showcase loaded** (`ActiveShowcase::None`), giving you a clean, empty RAG knowledge base without demo data, showcase tables, or showcase-specific query logic.

Each showcase directory contains **both its own unstructured PDF documents (`documents/`) and its own isolated Rust modules (`src/*.rs`)** (database schemas & seeding, HTTP endpoints, data generators, interactive OpenStreetMap view, and MCP tools). All analytical database queries are powered by the generic **Dynamic Text-to-SurrealQL Engine (`./rust-rag-orchestrator/src/text_to_sql.rs`)**, which uses the active showcase's schema prompt and Hop-1 PDF document anchors to dynamically generate and execute deterministic SurrealQL `SELECT` queries.

Use the **`--showcase`** CLI flag to load or clear any showcase at any time:

```bash
# List all available Showcases
./setup.sh --showcase list

# Load Showcase 01: IT Consulting, Client SLAs (BMW / Siemens) & HR Database (3,000 Employees / 300 Full Matches)
./setup.sh --showcase 1
# (Alternatively: ./setup.sh --showcase hr  or  ./setup.sh --showcase Showcase_01_HR_Consulting)

# Load Showcase 02: Logistics & Fleet Telemetry (120 Trucks, 232 GPS Tours in Mar/Apr & 5 Warehouses)
./setup.sh --showcase 2
# (Alternatively: ./setup.sh --showcase spedition  or  ./setup.sh --showcase Showcase_02_Spedition)

# Load Showcase 03: E-Commerce Webstore Multi-DB (Internal Contract DB + External Shop DB with Products & Sales History)
./setup.sh --showcase 3
# (Alternatively: ./setup.sh --showcase webstore  or  ./setup.sh --showcase Showcase_03_Webstore)

# Reset and clear all Showcase data (returns to empty knowledge base -> ActiveShowcase::None)
./setup.sh --showcase none
```

---

## 📂 Included Showcases & Module Architecture

| Directory | Domain & Topic | Isolated Rust Modules (`src/*.rs`) | Unstructured PDFs (`documents/`) | Structured SurrealDB SQL Tables | Guide & Test Prompts |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **[`Showcase_01_HR_Consulting/`](./Showcase_01_HR_Consulting/)** | **IT Consulting, SLA Compliance & HR Skill Matching** | `src/mod.rs`<br>`src/db.rs`<br>`src/handlers.rs`<br>`src/seed.rs`<br>`src/mcp.rs` | **7 PDFs** (`Sales/`: BMW AG, Siemens AG; `Projects/`: Alpha, Titan; `HR/`: 3 Profiles) | `employee_record` (**3,000 Employees** with **300 Full Matches** & **150 Training Gap Candidates**) | 👉 **[SHOWCASE.md](./Showcase_01_HR_Consulting/SHOWCASE.md)** |
| **[`Showcase_02_Spedition/`](./Showcase_02_Spedition/)** | **Logistics, Fleet Telemetry, 1-Hz GPS Tours & Warehouses** | `src/mod.rs`<br>`src/db.rs`<br>`src/handlers.rs`<br>`src/seed.rs`<br>`src/map_view.rs`<br>`src/mcp.rs` | **6 PDFs** (`Kunden_SLAs/`: BioPharma, ChemCorp ADR, NordFrost; `Wartung_Richtlinien/`; `Lagerhallen/`; `Qualitaet_Vorfaelle/`) | `fleet_vehicle` (**120 Trucks**), `fleet_tour` (**232 GPS Tours** incl. 605-point 1-Hz stream), `fleet_warehouse` (**5 Warehouses**) | 👉 **[SHOWCASE.md](./Showcase_02_Spedition/SHOWCASE.md)** |
| **[`Showcase_03_Webstore/`](./Showcase_03_Webstore/)** | **E-Commerce Webstore Multi-DB (Internal Contract DB + External Shop DB)** | `src/mod.rs`<br>`src/db.rs`<br>`src/handlers.rs`<br>`src/seed.rs`<br>`src/mcp.rs` | **6 PDFs** (`Liefervertraege/`: NordicAudio, AlpenTech; `Werbevertraege/`: PulseReach, AdVantage; `Mietvertraege_Lager/`: Hamburg, Munich) | **Internal DB**: `internal_contract_ledger` (6 Contracts)<br>**External DB** (`webstore_external / shop_erp`): `shop_product` (12 Products) & `shop_sale` (Sales History Mar & Apr) | 👉 **[SHOWCASE.md](./Showcase_03_Webstore/SHOWCASE.md)** |
