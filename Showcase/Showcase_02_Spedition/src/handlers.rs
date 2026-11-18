use super::seed::generate_demo_fleet_data;
use crate::{db::extract_result_array, showcases::ActiveShowcase, AppState};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;
use serde_json::json;
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct FleetSeedRequest {
    #[serde(default = "default_fleet_truck_count")]
    pub truck_count: usize,
}

fn default_fleet_truck_count() -> usize {
    120
}

/// Returns all fleet vehicles, tours (without raw 1-Hz GPS arrays), and warehouses (`GET /v1/fleet/overview`)
pub async fn list_fleet_overview_handler(State(state): State<AppState>) -> impl IntoResponse {
    let sql = "
        SELECT * FROM fleet_vehicle ORDER BY truck_id ASC LIMIT 5000;
        SELECT * OMIT gps_waypoints FROM fleet_tour ORDER BY departure_date ASC, tour_id ASC LIMIT 10000;
        SELECT * FROM fleet_warehouse ORDER BY warehouse_id ASC LIMIT 500;
    ";
    let res = state.db.execute_sql(sql).await.unwrap_or_default();
    let vehicles = extract_result_array(res.first());
    let tours = extract_result_array(res.get(1));
    let warehouses = extract_result_array(res.get(2));
    (
        StatusCode::OK,
        Json(json!({
            "vehicle_count": vehicles.len(),
            "tour_count": tours.len(),
            "warehouse_count": warehouses.len(),
            "vehicles": vehicles,
            "tours": tours,
            "warehouses": warehouses
        })),
    )
}

/// Clears all fleet vehicles, tours, and warehouses (`DELETE /v1/fleet/overview`)
pub async fn clear_fleet_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state
        .db
        .execute_sql("DELETE fleet_vehicle; DELETE fleet_tour; DELETE fleet_warehouse;")
        .await
    {
        Ok(_) => {
            state.set_active_showcase_if_matches(ActiveShowcase::Spedition, ActiveShowcase::None);
            (StatusCode::OK, Json(json!({ "status": "cleared" })))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

/// Seeds the Spedition / Logistics dataset (`POST /v1/fleet/seed`)
pub async fn seed_fleet_handler(
    State(state): State<AppState>,
    Json(req): Json<FleetSeedRequest>,
) -> impl IntoResponse {
    let target_trucks = req.truck_count.clamp(12, 500);
    let _ = state
        .db
        .execute_sql("DELETE fleet_vehicle; DELETE fleet_tour; DELETE fleet_warehouse;")
        .await;
    let (vehicles, tours, warehouses) = generate_demo_fleet_data(target_trucks);
    match state
        .db
        .upsert_fleet_data(&vehicles, &tours, &warehouses)
        .await
    {
        Ok((v_cnt, t_cnt, w_cnt)) => {
            state.set_active_showcase(ActiveShowcase::Spedition);
            info!(
                "Seeded Spedition showcase into SurrealDB (Showcase 02): {} trucks, {} GPS tours, {} warehouses.",
                v_cnt, t_cnt, w_cnt
            );
            (
                StatusCode::OK,
                Json(json!({
                    "status": "seeded",
                    "vehicle_count": v_cnt,
                    "tour_count": t_cnt,
                    "warehouse_count": w_cnt
                })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}
