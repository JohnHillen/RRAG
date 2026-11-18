use crate::db::{extract_result_array, sanitize_key, SurrealClient};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetVehicleRecord {
    pub truck_id: String,
    pub license_plate: String,
    pub model: String,
    pub vehicle_type: String,
    pub emission_class: String,
    pub assigned_driver: String,
    pub mileage_km: i64,
    pub next_inspection_due_km: i64,
    pub km_until_inspection: i64,
    pub next_inspection_date: String,
    pub inspection_status: String,
    pub oil_level_percent: i64,
    pub oil_status: String,
    pub adblue_level_percent: i64,
    pub brake_wear_percent: i64,
    pub tire_pressure_bar: f64,
    pub fuel_consumption_l_100km: f64,
    pub cooling_unit_status: String,
    #[serde(default)]
    pub adr_certified: bool,
    pub current_location: String,
    pub operational_status: String,
}

fn default_satellites() -> i64 {
    12
}
fn default_hdop() -> f64 {
    0.9
}
fn default_accuracy_m() -> f64 {
    1.8
}
fn default_fix_type() -> String {
    "3D-Fix".to_string()
}
fn default_weather() -> String {
    "☀️ Sonnig & Trocken".to_string()
}
fn default_ambient_temp() -> f64 {
    15.5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpsWaypoint {
    pub timestamp: String,
    pub lat: f64,
    pub lon: f64,
    pub location_name: String,
    pub speed_kmh: i64,
    #[serde(default)]
    pub temp_c: Option<f64>,
    #[serde(default = "default_satellites")]
    pub satellites_used: i64,
    #[serde(default = "default_hdop")]
    pub hdop: f64,
    #[serde(default = "default_accuracy_m")]
    pub gps_accuracy_m: f64,
    #[serde(default = "default_fix_type")]
    pub fix_type: String,
    #[serde(default = "default_weather")]
    pub weather_condition: String,
    #[serde(default = "default_ambient_temp")]
    pub ambient_temp_c: f64,
    #[serde(default)]
    pub precipitation_mm_h: f64,
    #[serde(default)]
    pub wind_speed_kmh: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetTourRecord {
    pub tour_id: String,
    pub truck_id: String,
    pub license_plate: String,
    pub driver_name: String,
    pub departure_date: String,
    pub arrival_date: String,
    pub month: String,
    pub origin_warehouse_id: String,
    pub origin_city: String,
    pub destination_warehouse_id: String,
    pub destination_city: String,
    pub customer_name: String,
    pub cargo_description: String,
    pub cargo_category: String,
    pub cargo_weight_tons: f64,
    pub distance_km: i64,
    pub gps_route: String,
    #[serde(default)]
    pub gps_waypoints: Vec<GpsWaypoint>,
    #[serde(default)]
    pub google_maps_url: String,
    #[serde(default)]
    pub recorded_temp_c: Option<f64>,
    #[serde(default)]
    pub delay_minutes: i64,
    pub delivery_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetWarehouseRecord {
    pub warehouse_id: String,
    pub name: String,
    pub city: String,
    pub address: String,
    pub gps_coords: String,
    pub storage_type: String,
    pub capacity_pallets: i64,
    pub utilization_percent: i64,
    #[serde(default)]
    pub certifications: Vec<String>,
    pub restrictions: String,
    pub manager_name: String,
}

impl SurrealClient {
    /// Ensures `fleet_vehicle`, `fleet_tour`, and `fleet_warehouse` tables exist in SurrealDB (only called when Showcase 02 is loaded).
    pub async fn ensure_fleet_schema(&self) -> Result<()> {
        let sql = "
            DEFINE TABLE IF NOT EXISTS fleet_vehicle SCHEMAFULL;
            DEFINE FIELD IF NOT EXISTS truck_id                 ON TABLE fleet_vehicle TYPE string;
            DEFINE FIELD IF NOT EXISTS license_plate            ON TABLE fleet_vehicle TYPE string;
            DEFINE FIELD IF NOT EXISTS model                    ON TABLE fleet_vehicle TYPE string;
            DEFINE FIELD IF NOT EXISTS vehicle_type             ON TABLE fleet_vehicle TYPE string;
            DEFINE FIELD IF NOT EXISTS emission_class           ON TABLE fleet_vehicle TYPE string;
            DEFINE FIELD IF NOT EXISTS assigned_driver          ON TABLE fleet_vehicle TYPE string;
            DEFINE FIELD IF NOT EXISTS mileage_km               ON TABLE fleet_vehicle TYPE int;
            DEFINE FIELD IF NOT EXISTS next_inspection_due_km   ON TABLE fleet_vehicle TYPE int;
            DEFINE FIELD IF NOT EXISTS km_until_inspection      ON TABLE fleet_vehicle TYPE int;
            DEFINE FIELD IF NOT EXISTS next_inspection_date     ON TABLE fleet_vehicle TYPE string;
            DEFINE FIELD IF NOT EXISTS inspection_status        ON TABLE fleet_vehicle TYPE string;
            DEFINE FIELD IF NOT EXISTS oil_level_percent        ON TABLE fleet_vehicle TYPE int;
            DEFINE FIELD IF NOT EXISTS oil_status               ON TABLE fleet_vehicle TYPE string;
            DEFINE FIELD IF NOT EXISTS adblue_level_percent     ON TABLE fleet_vehicle TYPE int;
            DEFINE FIELD IF NOT EXISTS brake_wear_percent       ON TABLE fleet_vehicle TYPE int;
            DEFINE FIELD IF NOT EXISTS tire_pressure_bar        ON TABLE fleet_vehicle TYPE float;
            DEFINE FIELD IF NOT EXISTS fuel_consumption_l_100km ON TABLE fleet_vehicle TYPE float;
            DEFINE FIELD IF NOT EXISTS cooling_unit_status      ON TABLE fleet_vehicle TYPE string;
            DEFINE FIELD IF NOT EXISTS adr_certified            ON TABLE fleet_vehicle TYPE bool DEFAULT false;
            DEFINE FIELD IF NOT EXISTS current_location         ON TABLE fleet_vehicle TYPE string;
            DEFINE FIELD IF NOT EXISTS operational_status       ON TABLE fleet_vehicle TYPE string;
            DEFINE FIELD IF NOT EXISTS updated_at               ON TABLE fleet_vehicle TYPE datetime DEFAULT time::now();
            DEFINE INDEX IF NOT EXISTS idx_fleet_truck_id       ON TABLE fleet_vehicle FIELDS truck_id UNIQUE;

            DEFINE TABLE IF NOT EXISTS fleet_tour SCHEMAFULL;
            DEFINE FIELD IF NOT EXISTS tour_id                  ON TABLE fleet_tour TYPE string;
            DEFINE FIELD IF NOT EXISTS truck_id                 ON TABLE fleet_tour TYPE string;
            DEFINE FIELD IF NOT EXISTS license_plate            ON TABLE fleet_tour TYPE string;
            DEFINE FIELD IF NOT EXISTS driver_name              ON TABLE fleet_tour TYPE string;
            DEFINE FIELD IF NOT EXISTS departure_date           ON TABLE fleet_tour TYPE string;
            DEFINE FIELD IF NOT EXISTS arrival_date             ON TABLE fleet_tour TYPE string;
            DEFINE FIELD IF NOT EXISTS month                    ON TABLE fleet_tour TYPE string;
            DEFINE FIELD IF NOT EXISTS origin_warehouse_id      ON TABLE fleet_tour TYPE string;
            DEFINE FIELD IF NOT EXISTS origin_city              ON TABLE fleet_tour TYPE string;
            DEFINE FIELD IF NOT EXISTS destination_warehouse_id ON TABLE fleet_tour TYPE string;
            DEFINE FIELD IF NOT EXISTS destination_city         ON TABLE fleet_tour TYPE string;
            DEFINE FIELD IF NOT EXISTS customer_name            ON TABLE fleet_tour TYPE string;
            DEFINE FIELD IF NOT EXISTS cargo_description        ON TABLE fleet_tour TYPE string;
            DEFINE FIELD IF NOT EXISTS cargo_category           ON TABLE fleet_tour TYPE string;
            DEFINE FIELD IF NOT EXISTS cargo_weight_tons        ON TABLE fleet_tour TYPE float;
            DEFINE FIELD IF NOT EXISTS distance_km              ON TABLE fleet_tour TYPE int;
            DEFINE FIELD IF NOT EXISTS gps_route                ON TABLE fleet_tour TYPE string;
            DEFINE FIELD IF NOT EXISTS gps_waypoints            ON TABLE fleet_tour TYPE option<array<object>> FLEXIBLE;
            DEFINE FIELD IF NOT EXISTS google_maps_url          ON TABLE fleet_tour TYPE option<string>;
            DEFINE FIELD IF NOT EXISTS recorded_temp_c          ON TABLE fleet_tour TYPE option<float>;
            DEFINE FIELD IF NOT EXISTS delay_minutes            ON TABLE fleet_tour TYPE int DEFAULT 0;
            DEFINE FIELD IF NOT EXISTS delivery_status          ON TABLE fleet_tour TYPE string;
            DEFINE FIELD IF NOT EXISTS updated_at               ON TABLE fleet_tour TYPE datetime DEFAULT time::now();
            DEFINE INDEX IF NOT EXISTS idx_fleet_tour_id        ON TABLE fleet_tour FIELDS tour_id UNIQUE;

            DEFINE TABLE IF NOT EXISTS fleet_warehouse SCHEMAFULL;
            DEFINE FIELD IF NOT EXISTS warehouse_id             ON TABLE fleet_warehouse TYPE string;
            DEFINE FIELD IF NOT EXISTS name                     ON TABLE fleet_warehouse TYPE string;
            DEFINE FIELD IF NOT EXISTS city                     ON TABLE fleet_warehouse TYPE string;
            DEFINE FIELD IF NOT EXISTS address                  ON TABLE fleet_warehouse TYPE string;
            DEFINE FIELD IF NOT EXISTS gps_coords               ON TABLE fleet_warehouse TYPE string;
            DEFINE FIELD IF NOT EXISTS storage_type             ON TABLE fleet_warehouse TYPE string;
            DEFINE FIELD IF NOT EXISTS capacity_pallets         ON TABLE fleet_warehouse TYPE int;
            DEFINE FIELD IF NOT EXISTS utilization_percent      ON TABLE fleet_warehouse TYPE int;
            DEFINE FIELD IF NOT EXISTS certifications           ON TABLE fleet_warehouse TYPE array<string>;
            DEFINE FIELD IF NOT EXISTS restrictions             ON TABLE fleet_warehouse TYPE string;
            DEFINE FIELD IF NOT EXISTS manager_name             ON TABLE fleet_warehouse TYPE string;
            DEFINE FIELD IF NOT EXISTS updated_at               ON TABLE fleet_warehouse TYPE datetime DEFAULT time::now();
            DEFINE INDEX IF NOT EXISTS idx_fleet_warehouse_id   ON TABLE fleet_warehouse FIELDS warehouse_id UNIQUE;
        ";
        self.execute_sql(sql).await?;
        Ok(())
    }

    /// Upserts fleet vehicles, tours, and warehouses in batches.
    pub async fn upsert_fleet_data(
        &self,
        vehicles: &[FleetVehicleRecord],
        tours: &[FleetTourRecord],
        warehouses: &[FleetWarehouseRecord],
    ) -> Result<(usize, usize, usize)> {
        self.ensure_fleet_schema().await?;

        for batch in vehicles.chunks(100) {
            let mut sql = String::from("BEGIN TRANSACTION;\n");
            for v in batch {
                let rk = sanitize_key(&v.truck_id);
                sql.push_str(&format!(
                    "UPSERT type::record('fleet_vehicle', {rk}) SET \
                        truck_id = {tid}, \
                        license_plate = {plate}, \
                        model = {model}, \
                        vehicle_type = {vtype}, \
                        emission_class = {emiss}, \
                        assigned_driver = {drv}, \
                        mileage_km = {km}, \
                        next_inspection_due_km = {insp_km}, \
                        km_until_inspection = {rem_km}, \
                        next_inspection_date = {insp_dt}, \
                        inspection_status = {insp_st}, \
                        oil_level_percent = {oil_pct}, \
                        oil_status = {oil_st}, \
                        adblue_level_percent = {adblue}, \
                        brake_wear_percent = {brake}, \
                        tire_pressure_bar = {tire}, \
                        fuel_consumption_l_100km = {fuel}, \
                        cooling_unit_status = {cool}, \
                        adr_certified = {adr}, \
                        current_location = {loc}, \
                        operational_status = {op_st}, \
                        updated_at = time::now();\n",
                    rk = serde_json::to_string(&rk)?,
                    tid = serde_json::to_string(&v.truck_id)?,
                    plate = serde_json::to_string(&v.license_plate)?,
                    model = serde_json::to_string(&v.model)?,
                    vtype = serde_json::to_string(&v.vehicle_type)?,
                    emiss = serde_json::to_string(&v.emission_class)?,
                    drv = serde_json::to_string(&v.assigned_driver)?,
                    km = v.mileage_km,
                    insp_km = v.next_inspection_due_km,
                    rem_km = v.km_until_inspection,
                    insp_dt = serde_json::to_string(&v.next_inspection_date)?,
                    insp_st = serde_json::to_string(&v.inspection_status)?,
                    oil_pct = v.oil_level_percent,
                    oil_st = serde_json::to_string(&v.oil_status)?,
                    adblue = v.adblue_level_percent,
                    brake = v.brake_wear_percent,
                    tire = v.tire_pressure_bar,
                    fuel = v.fuel_consumption_l_100km,
                    cool = serde_json::to_string(&v.cooling_unit_status)?,
                    adr = v.adr_certified,
                    loc = serde_json::to_string(&v.current_location)?,
                    op_st = serde_json::to_string(&v.operational_status)?,
                ));
            }
            sql.push_str("COMMIT TRANSACTION;\n");
            self.execute_sql(&sql).await?;
        }

        for batch in tours.chunks(150) {
            let mut sql = String::from("BEGIN TRANSACTION;\n");
            for t in batch {
                let rk = sanitize_key(&t.tour_id);
                let temp_str = match t.recorded_temp_c {
                    Some(val) => format!("{:.1}", val),
                    None => "NONE".to_string(),
                };
                sql.push_str(&format!(
                    "UPSERT type::record('fleet_tour', {rk}) SET \
                        tour_id = {tid}, \
                        truck_id = {trid}, \
                        license_plate = {plate}, \
                        driver_name = {drv}, \
                        departure_date = {dep}, \
                        arrival_date = {arr}, \
                        month = {mon}, \
                        origin_warehouse_id = {owh}, \
                        origin_city = {ocity}, \
                        destination_warehouse_id = {dwh}, \
                        destination_city = {dcity}, \
                        customer_name = {cust}, \
                        cargo_description = {cdesc}, \
                        cargo_category = {ccat}, \
                        cargo_weight_tons = {cwgt}, \
                        distance_km = {dist}, \
                        gps_route = {gps}, \
                        gps_waypoints = {gwps}, \
                        google_maps_url = {gmap}, \
                        recorded_temp_c = {temp}, \
                        delay_minutes = {dly}, \
                        delivery_status = {dst}, \
                        updated_at = time::now();\n",
                    rk = serde_json::to_string(&rk)?,
                    tid = serde_json::to_string(&t.tour_id)?,
                    trid = serde_json::to_string(&t.truck_id)?,
                    plate = serde_json::to_string(&t.license_plate)?,
                    drv = serde_json::to_string(&t.driver_name)?,
                    dep = serde_json::to_string(&t.departure_date)?,
                    arr = serde_json::to_string(&t.arrival_date)?,
                    mon = serde_json::to_string(&t.month)?,
                    owh = serde_json::to_string(&t.origin_warehouse_id)?,
                    ocity = serde_json::to_string(&t.origin_city)?,
                    dwh = serde_json::to_string(&t.destination_warehouse_id)?,
                    dcity = serde_json::to_string(&t.destination_city)?,
                    cust = serde_json::to_string(&t.customer_name)?,
                    cdesc = serde_json::to_string(&t.cargo_description)?,
                    ccat = serde_json::to_string(&t.cargo_category)?,
                    cwgt = t.cargo_weight_tons,
                    dist = t.distance_km,
                    gps = serde_json::to_string(&t.gps_route)?,
                    gwps = serde_json::to_string(&t.gps_waypoints)?,
                    gmap = serde_json::to_string(&t.google_maps_url)?,
                    temp = temp_str,
                    dly = t.delay_minutes,
                    dst = serde_json::to_string(&t.delivery_status)?,
                ));
            }
            sql.push_str("COMMIT TRANSACTION;\n");
            self.execute_sql(&sql).await?;
        }

        if !warehouses.is_empty() {
            let mut sql = String::from("BEGIN TRANSACTION;\n");
            for w in warehouses {
                let rk = sanitize_key(&w.warehouse_id);
                sql.push_str(&format!(
                    "UPSERT type::record('fleet_warehouse', {rk}) SET \
                        warehouse_id = {wid}, \
                        name = {name}, \
                        city = {city}, \
                        address = {addr}, \
                        gps_coords = {gps}, \
                        storage_type = {stype}, \
                        capacity_pallets = {cap}, \
                        utilization_percent = {util}, \
                        certifications = {certs}, \
                        restrictions = {restr}, \
                        manager_name = {mgr}, \
                        updated_at = time::now();\n",
                    rk = serde_json::to_string(&rk)?,
                    wid = serde_json::to_string(&w.warehouse_id)?,
                    name = serde_json::to_string(&w.name)?,
                    city = serde_json::to_string(&w.city)?,
                    addr = serde_json::to_string(&w.address)?,
                    gps = serde_json::to_string(&w.gps_coords)?,
                    stype = serde_json::to_string(&w.storage_type)?,
                    cap = w.capacity_pallets,
                    util = w.utilization_percent,
                    certs = serde_json::to_string(&w.certifications)?,
                    restr = serde_json::to_string(&w.restrictions)?,
                    mgr = serde_json::to_string(&w.manager_name)?,
                ));
            }
            sql.push_str("COMMIT TRANSACTION;\n");
            self.execute_sql(&sql).await?;
        }

        Ok((vehicles.len(), tours.len(), warehouses.len()))
    }

    /// Loads a single tour including its 1-Hz `gps_waypoints` for the interactive OpenStreetMap view (`map_view.rs`).
    pub async fn get_fleet_tour(&self, tour_id: &str) -> Result<Option<FleetTourRecord>> {
        let sql = format!(
            "SELECT tour_id, truck_id, license_plate, driver_name, departure_date, arrival_date, month, origin_warehouse_id, origin_city, destination_warehouse_id, destination_city, customer_name, cargo_description, cargo_category, cargo_weight_tons, distance_km, gps_route, gps_waypoints, google_maps_url, recorded_temp_c, delay_minutes, delivery_status FROM fleet_tour WHERE tour_id = {} LIMIT 1;",
            serde_json::to_string(tour_id)?
        );
        let res = self.execute_sql(&sql).await?;
        let items = extract_result_array(res.first());
        if let Some(first) = items.into_iter().next() {
            if let Ok(t) = serde_json::from_value::<FleetTourRecord>(first) {
                return Ok(Some(t));
            }
        }
        Ok(None)
    }
}
