pub mod db;
pub mod handlers;
pub mod map_view;
pub mod mcp;
pub mod seed;

use crate::{db::RetrievedChunk, text_to_sql::execute_dynamic_text_to_sql, AppState};
use axum::{
    routing::{get, post},
    Router,
};
use handlers::{clear_fleet_handler, list_fleet_overview_handler, seed_fleet_handler};
use map_view::get_fleet_tour_map_handler;

pub const SPEDITION_SCHEMA_PROMPT: &str = r#"
1) TABLE `fleet_tour` (GPS-Tracked Logistics Tours in March & April 2026):
   - `tour_id` (string): Unique Tour ID, e.g. 'TOUR-2026-0301', 'TOUR-2026-0402', 'TOUR-2026-0412', 'TOUR-2026-0415'
   - `truck_id` (string): Vehicle ID ('LKW-01' .. 'LKW-12' are the 12 core long-haul trucks; 'LKW-13' .. 'LKW-120' are regional shuttle trucks)
   - `license_plate` (string): License plate, e.g. 'M-SP-4011' (LKW-01), 'HH-LG-2042' (LKW-02), 'DU-CH-3093' (LKW-03), 'F-TH-4014' (LKW-04), 'M-EV-5055' (LKW-05), 'HH-AD-6066' (LKW-06), 'HH-LG-2077' (LKW-07), 'S-IN-8088' (LKW-08), 'B-TR-9019' (LKW-09), 'DU-CH-1020' (LKW-10), 'F-FR-1121' (LKW-11), 'M-SL-1222' (LKW-12)
   - `driver_name` (string): Driver full name, e.g. 'Andreas Kowalski', 'Murat Demir', 'Viktor Makarov', 'Stefan Lindemann', 'Elena Berger', 'Mehmet Yilmaz', 'Lukas Petersen', 'Marco Rossi', 'Florian Bichler', 'Timo Brandt', 'Katrin Baumann'
   - `departure_date` (string): ISO date '2026-03-04' .. '2026-04-28'
   - `arrival_date` (string): ISO date
   - `month` (string): Exact values are `'März 2026'` or `'April 2026'` (Filter April with `month = 'April 2026'`, March with `month = 'März 2026'`)
   - `origin_warehouse_id` (string): 'WH-HAM-01', 'WH-MUC-02', 'WH-FRA-03', 'WH-DUI-04', 'WH-BER-05'
   - `origin_city` (string): 'Hamburg', 'München', 'Frankfurt', 'Duisburg', 'Berlin'
   - `destination_warehouse_id` (string): 'WH-HAM-01', 'WH-MUC-02', 'WH-FRA-03', 'WH-DUI-04', 'WH-BER-05'
   - `destination_city` (string): 'Hamburg', 'München', 'Frankfurt', 'Duisburg', 'Berlin'
   - `customer_name` (string): Customer name, e.g. 'BioPharma Berlin GmbH', 'Charité & BioPharma Berlin GmbH', 'ChemCorp Rhein-Main AG', 'REWE Großhandel Ost & NordFrost', 'Airbus Operations Hamburg'
   - `cargo_description` (string): Cargo details
   - `cargo_category` (string): 'Pharma-Kühlkette', 'ADR-Gefahrgut', 'Lebensmittel-Frische', 'Schwerlast', 'Industrie & Automotive', 'Regional-Shuttle'
   - `cargo_weight_tons` (float): Weight in metric tons
   - `distance_km` (int): Driven tour distance in km
   - `gps_route` (string): Highway route summary
   - `recorded_temp_c` (option<float>): Recorded cooling temperature in °C (e.g. -19.8, -14.2)
   - `delay_minutes` (int): Delay in minutes (0 = on time)
   - `delivery_status` (string): Delivery & SLA status
   - `gps_waypoints` (array<object>): 1-Hz GPS telemetry points (`timestamp`, `lat`, `lon`, `location_name`, `speed_kmh`, `satellites_used`, `hdop`, `gps_accuracy_m`, `fix_type`, `weather_condition`, `ambient_temp_c`, `precipitation_mm_h`, `wind_speed_kmh`).
     -> IMPORTANT: Never select `gps_waypoints` unless the user specifically asks about GPS satellites, accuracy/drift, tunnels, or weather!

2) TABLE `fleet_vehicle` (120 Heavy-Duty Trucks — Vehicle Master Data & Maintenance Status):
   - `truck_id` (string): 'LKW-01' .. 'LKW-120'
   - `license_plate` (string): License plate
   - `model` (string): Truck model
   - `vehicle_type` (string): Trailer/body type
   - `emission_class` (string): 'Euro-6d', 'Euro-6c', 'Elektro (Zero-Emission)'
   - `assigned_driver` (string): Assigned primary driver
   - `mileage_km` (int): Total lifetime odometer reading in km (NOT monthly tour km!)
   - `next_inspection_due_km` (int): Odometer reading when next inspection is due
   - `km_until_inspection` (int): Remaining km until next inspection (<= 2500 means inspection due soon!)
   - `next_inspection_date` (string): Date of next inspection
   - `inspection_status` (string): 'OK', 'WARNUNG ...', 'KRITISCH ...'
   - `oil_level_percent` (int): Engine oil level in % (< 60% is warning/critical)
   - `oil_status` (string): Oil status text
   - `adblue_level_percent` (int): AdBlue level in %
   - `brake_wear_percent` (int): Brake wear in % (>= 70% is critical)
   - `tire_pressure_bar` (float): Tire pressure in bar
   - `fuel_consumption_l_100km` (float): Fuel consumption in L/100km
   - `cooling_unit_status` (string): Cooling unit status
   - `adr_certified` (bool): true if certified for ADR hazardous goods
   - `current_location` (string): Current depot/warehouse location
   - `operational_status` (string): 'Einsatzbereit', 'Eingeschränkt ...', 'Gesperrt ...'

3) TABLE `fleet_warehouse` (5 European Logistics Hubs & Warehouses):
   - `warehouse_id` (string): 'WH-HAM-01', 'WH-MUC-02', 'WH-FRA-03', 'WH-DUI-04', 'WH-BER-05'
   - `name` (string): Warehouse name
   - `city` (string): 'Hamburg', 'München', 'Frankfurt', 'Duisburg', 'Berlin'
   - `address` (string): Street address
   - `gps_coords` (string): GPS coordinates
   - `storage_type` (string): Storage specialization
   - `capacity_pallets` (int): Total pallet capacity
   - `utilization_percent` (int): Current utilization in %
   - `certifications` (array<string>): Warehouse certifications
   - `restrictions` (string): Environmental / ADR / operating restrictions
   - `manager_name` (string): Warehouse manager name

EXAMPLES:
- Drivers sorted descending by driven kilometers in April:
  `SELECT driver_name, truck_id, license_plate, math::sum(distance_km) AS total_km, count() AS tours, math::sum(cargo_weight_tons) AS total_tons FROM fleet_tour WHERE month = 'April 2026' GROUP BY driver_name, truck_id, license_plate ORDER BY total_km DESC LIMIT 20;`
- Deliveries to Berlin in April + vehicle status of those trucks:
  `SELECT tour_id, departure_date, truck_id, license_plate, driver_name, origin_city, destination_city, customer_name, cargo_description, distance_km, recorded_temp_c, delivery_status FROM fleet_tour WHERE destination_city = 'Berlin' AND month = 'April 2026' ORDER BY departure_date ASC;`
  `SELECT truck_id, license_plate, assigned_driver, mileage_km, km_until_inspection, next_inspection_date, oil_level_percent, brake_wear_percent, cooling_unit_status, operational_status FROM fleet_vehicle WHERE km_until_inspection <= 2500 OR oil_level_percent < 60 ORDER BY km_until_inspection ASC;`
- Tours for a specific license plate or truck (e.g. 'HH-LG-2077' or 'LKW-07'):
  `SELECT tour_id, departure_date, month, truck_id, license_plate, driver_name, origin_city, destination_city, customer_name, cargo_description, distance_km, delay_minutes, delivery_status FROM fleet_tour WHERE license_plate = 'HH-LG-2077' OR truck_id = 'LKW-07' ORDER BY departure_date ASC;`
"#;

/// Registers all HTTP routes belonging to Showcase 02 (Spedition, Fleet Telemetry & 1-Hz GPS Tours).
pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/fleet/overview",
            get(list_fleet_overview_handler).delete(clear_fleet_handler),
        )
        .route("/v1/fleet/seed", post(seed_fleet_handler))
        .route("/v1/fleet/tours/:tour_id/map", get(get_fleet_tour_map_handler))
}

/// Dynamically generates and executes SurrealQL via the LLM (`execute_dynamic_text_to_sql`) when Showcase 02 is active.
pub async fn augment_retrieval(
    state: &AppState,
    query: &str,
    mut ranked_pool: Vec<RetrievedChunk>,
    top_n: usize,
    db_ms: &mut u64,
    reranker_used: &mut String,
) -> Vec<RetrievedChunk> {
    let primary_anchor = ranked_pool
        .iter()
        .find(|c| {
            let t = c.doc_title.to_lowercase();
            t.contains("vereinbarung")
                || t.contains("vertrag")
                || t.contains("sla")
                || t.contains("richtlinie")
                || t.contains("audit")
        })
        .or_else(|| ranked_pool.first())
        .cloned();

    ranked_pool.truncate(top_n);
    let mut final_chunks = ranked_pool;

    let t_sql = std::time::Instant::now();
    if let Some(mut sql_chunk) = execute_dynamic_text_to_sql(
        state,
        query,
        primary_anchor.as_ref(),
        SPEDITION_SCHEMA_PROMPT,
        "fleet_sql_live_query_result",
        "fleet_logistics_tables",
        "🚛 [Speditions-DB Text-to-SQL]",
    )
    .await
    {
        *db_ms += t_sql.elapsed().as_millis() as u64;
        append_tour_map_links(&mut sql_chunk.content);
        let insert_idx = if final_chunks.is_empty() { 0 } else { 1 };
        final_chunks.insert(insert_idx, sql_chunk);
        if final_chunks.len() > (top_n + 1).max(3) {
            final_chunks.truncate((top_n + 1).max(3));
        }
        *reranker_used = format!("{} + 🧠 Text-to-SurrealQL (Speditions-DB)", reranker_used);
    }

    final_chunks
}

/// Appends clickable OpenStreetMap links (`[🗺️ Karte](/v1/fleet/tours/<tour_id>/map)`) for any `TOUR-YYYY-XXXX` ID found in the SQL table.
fn append_tour_map_links(md: &mut String) {
    let mut links = Vec::new();
    for token in md.split(|c: char| !c.is_ascii_alphanumeric() && c != '-') {
        if token.starts_with("TOUR-202") && token.len() == 14 {
            let link = format!("[🗺️ {} Karte](/v1/fleet/tours/{}/map)", token, token);
            if !links.contains(&link) {
                links.push(link);
            }
        }
    }
    if !links.is_empty() {
        md.push_str("\n**🗺️ Interaktive 1-Hz GPS-Karten-Links:** ");
        md.push_str(&links.join(" · "));
        md.push('\n');
    }
}
