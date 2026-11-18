use super::db::{FleetTourRecord, FleetVehicleRecord, FleetWarehouseRecord, GpsWaypoint};

/// Generates realistic Spedition data:
/// - 5 European logistics warehouses (`WH-HAM-01`, `WH-MUC-02`, `WH-FRA-03`, `WH-DUI-04`, `WH-BER-05`)
/// - `truck_count` heavy-duty trucks (`LKW-01` .. `LKW-XX`) with rich CAN-bus telemetry
/// - GPS-tracked tours across **March 2026 & April 2026**
pub fn generate_demo_fleet_data(
    truck_count: usize,
) -> (
    Vec<FleetVehicleRecord>,
    Vec<FleetTourRecord>,
    Vec<FleetWarehouseRecord>,
) {
    let warehouses = vec![
        FleetWarehouseRecord {
            warehouse_id: "WH-HAM-01".into(),
            name: "Zentrallager Hamburg-Hafen".into(),
            city: "Hamburg".into(),
            address: "Am Sandtorkai 50, 20457 Hamburg".into(),
            gps_coords: "53.5413°N, 9.9841°E".into(),
            storage_type: "Übersee-Container, Tiefkühl & Gefahrgut".into(),
            capacity_pallets: 12000,
            utilization_percent: 82,
            certifications: vec!["IFS Logistics".into(), "ADR".into(), "EU-GDP Pharma".into()],
            restrictions: "24/7 Betrieb, Schwerlastkran bis 40t".into(),
            manager_name: "Henrik Lindqvist".into(),
        },
        FleetWarehouseRecord {
            warehouse_id: "WH-MUC-02".into(),
            name: "Logistik-Hub München-Nord".into(),
            city: "München".into(),
            address: "Schleißheimer Str. 120, 85748 Garching-Hochbrück".into(),
            gps_coords: "48.2489°N, 11.6214°E".into(),
            storage_type: "Automotive & High-Tech Cross-Docking".into(),
            capacity_pallets: 8500,
            utilization_percent: 76,
            certifications: vec!["ISO 9001".into(), "TISAX Automotive".into()],
            restrictions: "Hochvolt-Schnellladesäulen für E-LKW vorhanden".into(),
            manager_name: "Markus Bichler".into(),
        },
        FleetWarehouseRecord {
            warehouse_id: "WH-FRA-03".into(),
            name: "Pharma- & AirCargo-Kühllager Frankfurt-Süd".into(),
            city: "Frankfurt".into(),
            address: "CargoCity Süd, Gebäude 532, 60549 Frankfurt am Main".into(),
            gps_coords: "50.0264°N, 8.5583°E".into(),
            storage_type: "GDP-Pharma-Kühllager (-25°C bis +8°C)".into(),
            capacity_pallets: 6000,
            utilization_percent: 91,
            certifications: vec![
                "EU-GDP Pharma".into(),
                "ISO 13485".into(),
                "AirCargo Security".into(),
            ],
            restrictions: "Zutritt nur mit GDP-Thermo-Zertifikat & Temperaturprotokoll".into(),
            manager_name: "Dr. Claudia Mertens".into(),
        },
        FleetWarehouseRecord {
            warehouse_id: "WH-DUI-04".into(),
            name: "Rhein-Ruhr Gefahrgut- & Schwerlast-Depot Duisburg".into(),
            city: "Duisburg".into(),
            address: "Krablerstraße 88, 47138 Duisburg-Ruhrort".into(),
            gps_coords: "51.4566°N, 6.7422°E".into(),
            storage_type: "ADR-Gefahrgut & Chemie-Sicherheitslager".into(),
            capacity_pallets: 9500,
            utilization_percent: 68,
            certifications: vec!["ADR Seveso-III".into(), "ISO 14001".into()],
            restrictions: "Auffangwannen für Gefahrstoffe ADR Klasse 3 & 9".into(),
            manager_name: "Frank Hufnagel".into(),
        },
        FleetWarehouseRecord {
            warehouse_id: "WH-BER-05".into(),
            name: "City-Hub Berlin-Westhafen".into(),
            city: "Berlin".into(),
            address: "Westhafenstraße 1, 13353 Berlin".into(),
            gps_coords: "52.5364°N, 13.3389°E".into(),
            storage_type: "Urban Distribution & Pharma-/Frische-Kühllager".into(),
            capacity_pallets: 5500,
            utilization_percent: 88,
            certifications: vec!["EU-GDP Pharma".into(), "IFS Food".into()],
            restrictions:
                "Berliner Umweltzone: Nur Euro-6d oder Elektro-LKW! Keine ADR-Klasse-3 Tanklagerung."
                    .into(),
            manager_name: "Sabine Krumm".into(),
        },
    ];

    let mut vehicles = vec![
        FleetVehicleRecord {
            truck_id: "LKW-01".into(),
            license_plate: "M-SP-4011".into(),
            model: "Mercedes-Benz Actros L 1851".into(),
            vehicle_type: "Kühlkoffer (GDP-Pharma -25°C)".into(),
            emission_class: "Euro-6d".into(),
            assigned_driver: "Andreas Kowalski".into(),
            mileage_km: 141200,
            next_inspection_due_km: 150000,
            km_until_inspection: 8800,
            next_inspection_date: "2026-07-15".into(),
            inspection_status: "OK".into(),
            oil_level_percent: 88,
            oil_status: "OK (Optimal)".into(),
            adblue_level_percent: 82,
            brake_wear_percent: 28,
            tire_pressure_bar: 8.5,
            fuel_consumption_l_100km: 26.4,
            cooling_unit_status: "Aktiv (-20.0°C GDP-Pharma zertifiziert)".into(),
            adr_certified: false,
            current_location: "Berlin-Westhafen (WH-BER-05)".into(),
            operational_status: "Einsatzbereit".into(),
        },
        FleetVehicleRecord {
            truck_id: "LKW-02".into(),
            license_plate: "HH-LG-2042".into(),
            model: "Scania R 450 A4x2NA".into(),
            vehicle_type: "Kühlkoffer (Frische & Tiefkühl)".into(),
            emission_class: "Euro-6d".into(),
            assigned_driver: "Stefan Lindemann".into(),
            mileage_km: 198650,
            next_inspection_due_km: 200000,
            km_until_inspection: 1350,
            next_inspection_date: "2026-05-20".into(),
            inspection_status: "WARNUNG (< 2.500 km - Nur Kurztouren)".into(),
            oil_level_percent: 54,
            oil_status: "WARNUNG (Gelb < 60% - Beim Depot-Stopp nachfüllen)".into(),
            adblue_level_percent: 64,
            brake_wear_percent: 76,
            tire_pressure_bar: 8.3,
            fuel_consumption_l_100km: 27.8,
            cooling_unit_status: "Aktiv (-18.0°C / +4.0°C Dual-Zone)".into(),
            adr_certified: false,
            current_location: "Zentrallager Hamburg-Hafen (WH-HAM-01)".into(),
            operational_status: "Eingeschränkt (Inspektion in 1.350 km)".into(),
        },
        FleetVehicleRecord {
            truck_id: "LKW-03".into(),
            license_plate: "DU-CH-3093".into(),
            model: "Volvo FH 540 Globetrotter".into(),
            vehicle_type: "Gefahrgut-Sattelzug (ADR Klasse 3/9)".into(),
            emission_class: "Euro-6d".into(),
            assigned_driver: "Mehmet Yilmaz".into(),
            mileage_km: 87400,
            next_inspection_due_km: 100000,
            km_until_inspection: 12600,
            next_inspection_date: "2026-09-10".into(),
            inspection_status: "OK".into(),
            oil_level_percent: 92,
            oil_status: "OK (Optimal)".into(),
            adblue_level_percent: 90,
            brake_wear_percent: 22,
            tire_pressure_bar: 8.6,
            fuel_consumption_l_100km: 28.1,
            cooling_unit_status: "Kein Kühlaggregat".into(),
            adr_certified: true,
            current_location: "Gefahrgut-Depot Duisburg (WH-DUI-04)".into(),
            operational_status: "Einsatzbereit".into(),
        },
        FleetVehicleRecord {
            truck_id: "LKW-04".into(),
            license_plate: "F-TH-4014".into(),
            model: "Mercedes-Benz Actros 1848 LS".into(),
            vehicle_type: "Kühlkoffer (GDP-Pharma -25°C)".into(),
            emission_class: "Euro-6d".into(),
            assigned_driver: "Viktor Makarov".into(),
            mileage_km: 173900,
            next_inspection_due_km: 175000,
            km_until_inspection: 1100,
            next_inspection_date: "2026-05-12".into(),
            inspection_status: "WARNUNG (< 2.500 km - Inspektion fällig)".into(),
            oil_level_percent: 38,
            oil_status: "KRITISCH (Rot < 45% - Sofortiger Öl-Stopp!)".into(),
            adblue_level_percent: 19,
            brake_wear_percent: 74,
            tire_pressure_bar: 7.4,
            fuel_consumption_l_100km: 29.5,
            cooling_unit_status: "DEFEKT (Thermostatventil-Warnung -14.2°C)".into(),
            adr_certified: false,
            current_location: "Pharma-Kühllager Frankfurt-Süd (WH-FRA-03)".into(),
            operational_status: "Gesperrt / Werkstattpflicht (Kühlaggregat & Öl)".into(),
        },
        FleetVehicleRecord {
            truck_id: "LKW-05".into(),
            license_plate: "M-EV-5055".into(),
            model: "Volvo FH Electric 4x2".into(),
            vehicle_type: "Elektro-Kühlkoffer (GDP-Pharma -25°C)".into(),
            emission_class: "Elektro (Zero-Emission)".into(),
            assigned_driver: "Katrin Baumann".into(),
            mileage_km: 64100,
            next_inspection_due_km: 100000,
            km_until_inspection: 35900,
            next_inspection_date: "2026-11-01".into(),
            inspection_status: "OK".into(),
            oil_level_percent: 96,
            oil_status: "OK (Getriebeöl optimal)".into(),
            adblue_level_percent: 100,
            brake_wear_percent: 18,
            tire_pressure_bar: 8.5,
            fuel_consumption_l_100km: 0.0,
            cooling_unit_status: "Aktiv (-21.0°C GDP-Pharma zertifiziert)".into(),
            adr_certified: false,
            current_location: "Logistik-Hub München-Nord (WH-MUC-02)".into(),
            operational_status: "Einsatzbereit".into(),
        },
        FleetVehicleRecord {
            truck_id: "LKW-06".into(),
            license_plate: "HH-AD-6066".into(),
            model: "MAN TGX 18.510 ADR".into(),
            vehicle_type: "Gefahrgut-Sattelzug (ADR Klasse 3/9)".into(),
            emission_class: "Euro-6d".into(),
            assigned_driver: "Lukas Petersen".into(),
            mileage_km: 118500,
            next_inspection_due_km: 150000,
            km_until_inspection: 31500,
            next_inspection_date: "2026-10-15".into(),
            inspection_status: "OK".into(),
            oil_level_percent: 84,
            oil_status: "OK (Optimal)".into(),
            adblue_level_percent: 75,
            brake_wear_percent: 38,
            tire_pressure_bar: 8.5,
            fuel_consumption_l_100km: 27.9,
            cooling_unit_status: "Kein Kühlaggregat".into(),
            adr_certified: true,
            current_location: "Zentrallager Hamburg-Hafen (WH-HAM-01)".into(),
            operational_status: "Einsatzbereit".into(),
        },
        FleetVehicleRecord {
            truck_id: "LKW-07".into(),
            license_plate: "HH-LG-2077".into(),
            model: "MAN TGX 18.510 Mega".into(),
            vehicle_type: "Schwerlast / Mega-Trailer".into(),
            emission_class: "Euro-6c".into(),
            assigned_driver: "Murat Demir".into(),
            mileage_km: 249820,
            next_inspection_due_km: 250000,
            km_until_inspection: 180,
            next_inspection_date: "2026-05-02".into(),
            inspection_status: "KRITISCH / SOFORT FÄLLIG (Nur 180 km Rest!)".into(),
            oil_level_percent: 34,
            oil_status: "KRITISCH (Rot 34% - Akute Motorschadensgefahr!)".into(),
            adblue_level_percent: 14,
            brake_wear_percent: 88,
            tire_pressure_bar: 7.2,
            fuel_consumption_l_100km: 32.4,
            cooling_unit_status: "Kein Kühlaggregat".into(),
            adr_certified: false,
            current_location: "Zentralwerkstatt Hamburg (WH-HAM-01)".into(),
            operational_status: "Gesperrt / Werkstattpflicht (Inspektion, Öl & Bremsen)".into(),
        },
        FleetVehicleRecord {
            truck_id: "LKW-08".into(),
            license_plate: "S-IN-8088".into(),
            model: "DAF XG+ 530 FT".into(),
            vehicle_type: "Standard-Planensattelzug (Industrie)".into(),
            emission_class: "Euro-6d".into(),
            assigned_driver: "Timo Brandt".into(),
            mileage_km: 132400,
            next_inspection_due_km: 150000,
            km_until_inspection: 17600,
            next_inspection_date: "2026-08-18".into(),
            inspection_status: "OK".into(),
            oil_level_percent: 79,
            oil_status: "OK (Optimal)".into(),
            adblue_level_percent: 68,
            brake_wear_percent: 44,
            tire_pressure_bar: 8.4,
            fuel_consumption_l_100km: 25.9,
            cooling_unit_status: "Kein Kühlaggregat".into(),
            adr_certified: false,
            current_location: "Logistik-Hub München-Nord (WH-MUC-02)".into(),
            operational_status: "Einsatzbereit".into(),
        },
        FleetVehicleRecord {
            truck_id: "LKW-09".into(),
            license_plate: "B-TR-9019".into(),
            model: "Scania S 500 Highline".into(),
            vehicle_type: "Kühlkoffer (GDP-Pharma -25°C)".into(),
            emission_class: "Euro-6d".into(),
            assigned_driver: "Elena Berger".into(),
            mileage_km: 91300,
            next_inspection_due_km: 100000,
            km_until_inspection: 8700,
            next_inspection_date: "2026-08-05".into(),
            inspection_status: "OK".into(),
            oil_level_percent: 91,
            oil_status: "OK (Optimal)".into(),
            adblue_level_percent: 88,
            brake_wear_percent: 25,
            tire_pressure_bar: 8.5,
            fuel_consumption_l_100km: 26.1,
            cooling_unit_status: "Aktiv (-20.5°C GDP-Pharma zertifiziert)".into(),
            adr_certified: false,
            current_location: "City-Hub Berlin-Westhafen (WH-BER-05)".into(),
            operational_status: "Einsatzbereit".into(),
        },
        FleetVehicleRecord {
            truck_id: "LKW-10".into(),
            license_plate: "DU-CH-1020".into(),
            model: "Mercedes-Benz Actros 1845 ADR".into(),
            vehicle_type: "Gefahrgut-Sattelzug (ADR Klasse 3/9)".into(),
            emission_class: "Euro-6d".into(),
            assigned_driver: "Christian Hummels".into(),
            mileage_km: 148100,
            next_inspection_due_km: 150000,
            km_until_inspection: 1900,
            next_inspection_date: "2026-05-25".into(),
            inspection_status: "WARNUNG (< 2.500 km - Gesperrt für ADR)".into(),
            oil_level_percent: 62,
            oil_status: "OK (Aber unter 70% ChemCorp-ADR-Limit)".into(),
            adblue_level_percent: 55,
            brake_wear_percent: 58,
            tire_pressure_bar: 8.1,
            fuel_consumption_l_100km: 28.7,
            cooling_unit_status: "Kein Kühlaggregat".into(),
            adr_certified: true,
            current_location: "Gefahrgut-Depot Duisburg (WH-DUI-04)".into(),
            operational_status: "Eingeschränkt (Bremsen > 50% & Inspektion < 3.000 km)".into(),
        },
        FleetVehicleRecord {
            truck_id: "LKW-11".into(),
            license_plate: "F-FR-1121".into(),
            model: "IVECO S-Way 490 LNG".into(),
            vehicle_type: "Standard-Planensattelzug (Konsumgüter)".into(),
            emission_class: "Euro-6d".into(),
            assigned_driver: "Marco Rossi".into(),
            mileage_km: 211000,
            next_inspection_due_km: 250000,
            km_until_inspection: 39000,
            next_inspection_date: "2026-10-30".into(),
            inspection_status: "OK".into(),
            oil_level_percent: 81,
            oil_status: "OK (Optimal)".into(),
            adblue_level_percent: 72,
            brake_wear_percent: 39,
            tire_pressure_bar: 8.5,
            fuel_consumption_l_100km: 24.8,
            cooling_unit_status: "Kein Kühlaggregat".into(),
            adr_certified: false,
            current_location: "Pharma-Kühllager Frankfurt-Süd (WH-FRA-03)".into(),
            operational_status: "Einsatzbereit".into(),
        },
        FleetVehicleRecord {
            truck_id: "LKW-12".into(),
            license_plate: "M-SL-1222".into(),
            model: "MAN TGX 26.580 6x2".into(),
            vehicle_type: "Schwerlast / Mega-Trailer".into(),
            emission_class: "Euro-6d".into(),
            assigned_driver: "Florian Bichler".into(),
            mileage_km: 199550,
            next_inspection_due_km: 200000,
            km_until_inspection: 450,
            next_inspection_date: "2026-05-08".into(),
            inspection_status: "KRITISCH / SOFORT FÄLLIG (Nur 450 km Rest!)".into(),
            oil_level_percent: 48,
            oil_status: "WARNUNG (Gelb 48% - Nachfüllen erforderlich)".into(),
            adblue_level_percent: 31,
            brake_wear_percent: 81,
            tire_pressure_bar: 8.2,
            fuel_consumption_l_100km: 31.2,
            cooling_unit_status: "Kein Kühlaggregat".into(),
            adr_certified: false,
            current_location: "Logistik-Hub München-Nord (WH-MUC-02)".into(),
            operational_status: "Gesperrt / Werkstattpflicht (Inspektion < 500 km)".into(),
        },
    ];

    let extra_first = [
        "Alexander", "Dennis", "Svenja", "Tomasz", "Kemal", "Patrick", "Mirko", "Sebastian",
        "Jan", "Matthias", "Tobias", "Nadine", "Daniel", "Marcel", "Philipp",
    ];
    let extra_last = [
        "König", "Schuster", "Hartmann", "Nowak", "Özdemir", "Hofmann", "Vukovic", "Weber",
        "Krämer", "Richter", "Seidel", "Lorenz", "Winter", "Bauer", "Vogt",
    ];
    let extra_models = [
        (
            "Mercedes-Benz Actros L 1851",
            "Kühlkoffer (GDP-Pharma -25°C)",
            "Euro-6d",
            "Aktiv (-20.0°C GDP-Pharma zertifiziert)",
            false,
        ),
        (
            "MAN TGX 18.510",
            "Standard-Planensattelzug",
            "Euro-6d",
            "Kein Kühlaggregat",
            false,
        ),
        (
            "Scania R 450",
            "Kühlkoffer (Frische & Tiefkühl)",
            "Euro-6d",
            "Aktiv (-18.0°C Dual-Zone)",
            false,
        ),
        (
            "Volvo FH 540 ADR",
            "Gefahrgut-Sattelzug (ADR Klasse 3/9)",
            "Euro-6d",
            "Kein Kühlaggregat",
            true,
        ),
    ];
    for idx in 13..=truck_count {
        let (mdl, vtype, emiss, cool, adr) = extra_models[idx % extra_models.len()];
        let offset = idx - 13;
        let drv = format!(
            "{} {}",
            extra_first[offset % extra_first.len()],
            extra_last[(offset / extra_first.len() + offset * 4) % extra_last.len()]
        );
        let base_km = 50000 + ((idx * 7300) % 180000) as i64;
        let next_due = ((base_km / 50000) + 1) * 50000;
        let rem = (next_due - base_km).max(3000);
        let oil = 68 + ((idx * 7) % 28) as i64;
        let wh = &warehouses[idx % warehouses.len()];
        vehicles.push(FleetVehicleRecord {
            truck_id: format!("LKW-{:02}", idx),
            license_plate: format!("HH-FL-{:04}", 4000 + idx),
            model: mdl.into(),
            vehicle_type: vtype.into(),
            emission_class: emiss.into(),
            assigned_driver: drv.into(),
            mileage_km: base_km,
            next_inspection_due_km: next_due,
            km_until_inspection: rem,
            next_inspection_date: format!("2026-08-{:02}", (idx % 28) + 1),
            inspection_status: "OK".into(),
            oil_level_percent: oil,
            oil_status: "OK (Optimal)".into(),
            adblue_level_percent: 60 + ((idx * 5) % 35) as i64,
            brake_wear_percent: 20 + ((idx * 3) % 45) as i64,
            tire_pressure_bar: 8.5,
            fuel_consumption_l_100km: 26.5,
            cooling_unit_status: cool.into(),
            adr_certified: adr,
            current_location: format!("{} ({})", wh.name, wh.warehouse_id),
            operational_status: "Einsatzbereit".into(),
        });
    }

    let mut tours = vec![
        // --- MARCH 2026 TOURS ---
        FleetTourRecord {
            tour_id: "TOUR-2026-0301".into(),
            truck_id: "LKW-01".into(),
            license_plate: "M-SP-4011".into(),
            driver_name: "Andreas Kowalski".into(),
            departure_date: "2026-03-04".into(),
            arrival_date: "2026-03-05".into(),
            month: "März 2026".into(),
            origin_warehouse_id: "WH-FRA-03".into(),
            origin_city: "Frankfurt".into(),
            destination_warehouse_id: "WH-MUC-02".into(),
            destination_city: "München".into(),
            customer_name: "Klinikum Großhadern München".into(),
            cargo_description: "Onkologie-Medikamente (Kühlkette -20°C)".into(),
            cargo_category: "Pharma-Kühlkette".into(),
            cargo_weight_tons: 14.8,
            distance_km: 392,
            gps_route:
                "Frankfurt (50.03°N, 8.56°E) -> A3 -> Würzburg -> A9 -> München-Nord (48.25°N, 11.62°E)"
                    .into(),
            gps_waypoints: Vec::new(),
            google_maps_url: String::new(),
            recorded_temp_c: Some(-20.1),
            delay_minutes: 0,
            delivery_status: "Pünktlich & SLA-konform".into(),
        },
        FleetTourRecord {
            tour_id: "TOUR-2026-0305".into(),
            truck_id: "LKW-09".into(),
            license_plate: "B-TR-9019".into(),
            driver_name: "Elena Berger".into(),
            departure_date: "2026-03-11".into(),
            arrival_date: "2026-03-11".into(),
            month: "März 2026".into(),
            origin_warehouse_id: "WH-HAM-01".into(),
            origin_city: "Hamburg".into(),
            destination_warehouse_id: "WH-BER-05".into(),
            destination_city: "Berlin".into(),
            customer_name: "BioPharma Berlin GmbH".into(),
            cargo_description: "Impfstoff-Rohstoffe & Blutplasma (Kühlkette -20°C)".into(),
            cargo_category: "Pharma-Kühlkette".into(),
            cargo_weight_tons: 15.6,
            distance_km: 289,
            gps_route:
                "Hamburg-Hafen (53.54°N, 9.98°E) -> A24 -> Wittstock -> A10 -> Berlin-Westhafen (52.54°N, 13.34°E)"
                    .into(),
            gps_waypoints: Vec::new(),
            google_maps_url: String::new(),
            recorded_temp_c: Some(-19.9),
            delay_minutes: 0,
            delivery_status: "Pünktlich & SLA-konform".into(),
        },
        FleetTourRecord {
            tour_id: "TOUR-2026-0310".into(),
            truck_id: "LKW-03".into(),
            license_plate: "DU-CH-3093".into(),
            driver_name: "Mehmet Yilmaz".into(),
            departure_date: "2026-03-15".into(),
            arrival_date: "2026-03-16".into(),
            month: "März 2026".into(),
            origin_warehouse_id: "WH-DUI-04".into(),
            origin_city: "Duisburg".into(),
            destination_warehouse_id: "WH-HAM-01".into(),
            destination_city: "Hamburg".into(),
            customer_name: "ChemCorp Rhein-Main AG".into(),
            cargo_description: "Spezial-Lösungsmittel ADR Klasse 3".into(),
            cargo_category: "ADR-Gefahrgut".into(),
            cargo_weight_tons: 22.4,
            distance_km: 380,
            gps_route:
                "Duisburg-Ruhrort (51.46°N, 6.74°E) -> A2 -> A1 -> Bremen -> Hamburg-Hafen (53.54°N, 9.98°E)"
                    .into(),
            gps_waypoints: Vec::new(),
            google_maps_url: String::new(),
            recorded_temp_c: None,
            delay_minutes: 15,
            delivery_status: "Pünktlich & SLA-konform".into(),
        },
        FleetTourRecord {
            tour_id: "TOUR-2026-0314".into(),
            truck_id: "LKW-08".into(),
            license_plate: "S-IN-8088".into(),
            driver_name: "Timo Brandt".into(),
            departure_date: "2026-03-19".into(),
            arrival_date: "2026-03-20".into(),
            month: "März 2026".into(),
            origin_warehouse_id: "WH-MUC-02".into(),
            origin_city: "München".into(),
            destination_warehouse_id: "WH-BER-05".into(),
            destination_city: "Berlin".into(),
            customer_name: "Siemens Energy Berlin".into(),
            cargo_description: "Schaltanlagen & Leistungselektronik".into(),
            cargo_category: "Industrie & Automotive".into(),
            cargo_weight_tons: 19.2,
            distance_km: 585,
            gps_route: "München-Nord -> A9 -> Nürnberg -> Leipzig -> A10 -> Berlin-Westhafen"
                .into(),
            gps_waypoints: Vec::new(),
            google_maps_url: String::new(),
            recorded_temp_c: None,
            delay_minutes: 0,
            delivery_status: "Pünktlich & SLA-konform".into(),
        },
        FleetTourRecord {
            tour_id: "TOUR-2026-0319".into(),
            truck_id: "LKW-07".into(),
            license_plate: "HH-LG-2077".into(),
            driver_name: "Murat Demir".into(),
            departure_date: "2026-03-24".into(),
            arrival_date: "2026-03-25".into(),
            month: "März 2026".into(),
            origin_warehouse_id: "WH-HAM-01".into(),
            origin_city: "Hamburg".into(),
            destination_warehouse_id: "WH-DUI-04".into(),
            destination_city: "Duisburg".into(),
            customer_name: "ThyssenStahl Logistik".into(),
            cargo_description: "Schwerlast-Stahlcoils & Walzprofile".into(),
            cargo_category: "Schwerlast".into(),
            cargo_weight_tons: 31.5,
            distance_km: 380,
            gps_route: "Hamburg-Hafen -> A1 -> Münster -> A2 -> Duisburg-Ruhrort".into(),
            gps_waypoints: Vec::new(),
            google_maps_url: String::new(),
            recorded_temp_c: None,
            delay_minutes: 40,
            delivery_status: "Geliefert (Erhöhter Bremsverschleiß gemeldet)".into(),
        },
        FleetTourRecord {
            tour_id: "TOUR-2026-0325".into(),
            truck_id: "LKW-05".into(),
            license_plate: "M-EV-5055".into(),
            driver_name: "Katrin Baumann".into(),
            departure_date: "2026-03-28".into(),
            arrival_date: "2026-03-28".into(),
            month: "März 2026".into(),
            origin_warehouse_id: "WH-MUC-02".into(),
            origin_city: "München".into(),
            destination_warehouse_id: "WH-FRA-03".into(),
            destination_city: "Frankfurt".into(),
            customer_name: "Sanofi Aventis Frankfurt".into(),
            cargo_description: "Insulin-Chargen (Kühlkette -20°C)".into(),
            cargo_category: "Pharma-Kühlkette".into(),
            cargo_weight_tons: 13.4,
            distance_km: 392,
            gps_route: "München-Nord -> A9 -> A3 -> Ladepark Hengersberg -> Frankfurt-Süd".into(),
            gps_waypoints: Vec::new(),
            google_maps_url: String::new(),
            recorded_temp_c: Some(-20.8),
            delay_minutes: 0,
            delivery_status: "Pünktlich & Zero-Emission SLA-konform".into(),
        },
        // --- APRIL 2026 TOURS ---
        FleetTourRecord {
            tour_id: "TOUR-2026-0402".into(),
            truck_id: "LKW-01".into(),
            license_plate: "M-SP-4011".into(),
            driver_name: "Andreas Kowalski".into(),
            departure_date: "2026-04-04".into(),
            arrival_date: "2026-04-05".into(),
            month: "April 2026".into(),
            origin_warehouse_id: "WH-FRA-03".into(),
            origin_city: "Frankfurt".into(),
            destination_warehouse_id: "WH-BER-05".into(),
            destination_city: "Berlin".into(),
            customer_name: "BioPharma Berlin GmbH".into(),
            cargo_description: "mRNA-Impfstoffe & Biologika (Kühlkette -20°C)".into(),
            cargo_category: "Pharma-Kühlkette".into(),
            cargo_weight_tons: 16.5,
            distance_km: 548,
            gps_route:
                "Frankfurt-Süd (50.03°N, 8.56°E) -> A5 -> A4 -> Erfurt -> A9 -> A10 -> Berlin-Westhafen (52.54°N, 13.34°E)"
                    .into(),
            gps_waypoints: Vec::new(),
            google_maps_url: String::new(),
            recorded_temp_c: Some(-19.8),
            delay_minutes: 0,
            delivery_status: "Pünktlich & SLA-konform".into(),
        },
        FleetTourRecord {
            tour_id: "TOUR-2026-0405".into(),
            truck_id: "LKW-02".into(),
            license_plate: "HH-LG-2042".into(),
            driver_name: "Stefan Lindemann".into(),
            departure_date: "2026-04-09".into(),
            arrival_date: "2026-04-09".into(),
            month: "April 2026".into(),
            origin_warehouse_id: "WH-HAM-01".into(),
            origin_city: "Hamburg".into(),
            destination_warehouse_id: "WH-BER-05".into(),
            destination_city: "Berlin".into(),
            customer_name: "REWE Großhandel Ost & NordFrost".into(),
            cargo_description: "Tiefkühlkost & Frische-Molkereiprodukte (-18°C)".into(),
            cargo_category: "Lebensmittel-Frische".into(),
            cargo_weight_tons: 21.0,
            distance_km: 289,
            gps_route:
                "Hamburg-Hafen (53.54°N, 9.98°E) -> A24 -> Dreieck Wittstock -> A10 -> Berlin-Westhafen (52.54°N, 13.34°E)"
                    .into(),
            gps_waypoints: Vec::new(),
            google_maps_url: String::new(),
            recorded_temp_c: Some(-18.4),
            delay_minutes: 10,
            delivery_status: "Pünktlich & SLA-konform".into(),
        },
        FleetTourRecord {
            tour_id: "TOUR-2026-0412".into(),
            truck_id: "LKW-04".into(),
            license_plate: "F-TH-4014".into(),
            driver_name: "Viktor Makarov".into(),
            departure_date: "2026-04-14".into(),
            arrival_date: "2026-04-15".into(),
            month: "April 2026".into(),
            origin_warehouse_id: "WH-FRA-03".into(),
            origin_city: "Frankfurt".into(),
            destination_warehouse_id: "WH-BER-05".into(),
            destination_city: "Berlin".into(),
            customer_name: "BioPharma Berlin GmbH".into(),
            cargo_description: "Sensible Blutplasma-Konzentrate (Kühlkette -20°C)".into(),
            cargo_category: "Pharma-Kühlkette".into(),
            cargo_weight_tons: 14.2,
            distance_km: 548,
            gps_route:
                "Frankfurt-Süd (50.03°N, 8.56°E) -> A5 -> A7 -> A2 -> Magdeburg -> A10 -> Berlin-Westhafen (52.54°N, 13.34°E)"
                    .into(),
            gps_waypoints: Vec::new(),
            google_maps_url: String::new(),
            recorded_temp_c: Some(-14.2),
            delay_minutes: 65,
            delivery_status:
                "SLA-VERSTOSS: Kühltemperatur auf -14.2°C gestiegen (Thermostatventil defekt) + 65 Min Verspätung"
                    .into(),
        },
        FleetTourRecord {
            tour_id: "TOUR-2026-0418".into(),
            truck_id: "LKW-06".into(),
            license_plate: "HH-AD-6066".into(),
            driver_name: "Lukas Petersen".into(),
            departure_date: "2026-04-19".into(),
            arrival_date: "2026-04-20".into(),
            month: "April 2026".into(),
            origin_warehouse_id: "WH-DUI-04".into(),
            origin_city: "Duisburg".into(),
            destination_warehouse_id: "WH-BER-05".into(),
            destination_city: "Berlin".into(),
            customer_name: "Siemens Energy & BatteryTech Berlin".into(),
            cargo_description: "Lithium-Ionen-Hochvoltspeicher (ADR Klasse 9)".into(),
            cargo_category: "ADR-Gefahrgut".into(),
            cargo_weight_tons: 19.8,
            distance_km: 525,
            gps_route:
                "Duisburg-Ruhrort (51.46°N, 6.74°E) -> A2 -> Hannover -> Braunschweig -> A10 -> Berlin-Westhafen (52.54°N, 13.34°E)"
                    .into(),
            gps_waypoints: Vec::new(),
            google_maps_url: String::new(),
            recorded_temp_c: None,
            delay_minutes: 0,
            delivery_status: "Pünktlich & ADR-konform".into(),
        },
        FleetTourRecord {
            tour_id: "TOUR-2026-0423".into(),
            truck_id: "LKW-09".into(),
            license_plate: "B-TR-9019".into(),
            driver_name: "Elena Berger".into(),
            departure_date: "2026-04-24".into(),
            arrival_date: "2026-04-24".into(),
            month: "April 2026".into(),
            origin_warehouse_id: "WH-HAM-01".into(),
            origin_city: "Hamburg".into(),
            destination_warehouse_id: "WH-BER-05".into(),
            destination_city: "Berlin".into(),
            customer_name: "BioPharma Berlin GmbH".into(),
            cargo_description: "Klinische Labor-Diagnostika & Impfstoffe (-20°C)".into(),
            cargo_category: "Pharma-Kühlkette".into(),
            cargo_weight_tons: 15.0,
            distance_km: 289,
            gps_route:
                "Hamburg-Hafen (53.54°N, 9.98°E) -> A24 -> Neuruppin -> A111 -> Berlin-Westhafen (52.54°N, 13.34°E)"
                    .into(),
            gps_waypoints: Vec::new(),
            google_maps_url: String::new(),
            recorded_temp_c: Some(-20.2),
            delay_minutes: 0,
            delivery_status: "Pünktlich & SLA-konform".into(),
        },
        FleetTourRecord {
            tour_id: "TOUR-2026-0428".into(),
            truck_id: "LKW-01".into(),
            license_plate: "M-SP-4011".into(),
            driver_name: "Andreas Kowalski".into(),
            departure_date: "2026-04-28".into(),
            arrival_date: "2026-04-29".into(),
            month: "April 2026".into(),
            origin_warehouse_id: "WH-MUC-02".into(),
            origin_city: "München".into(),
            destination_warehouse_id: "WH-BER-05".into(),
            destination_city: "Berlin".into(),
            customer_name: "Charité & BioPharma Berlin GmbH".into(),
            cargo_description: "Medizintechnik & sterile Thermo-Reagenzien (-20°C)".into(),
            cargo_category: "Pharma-Kühlkette".into(),
            cargo_weight_tons: 17.2,
            distance_km: 585,
            gps_route:
                "München-Nord (48.25°N, 11.62°E) -> A9 -> Nürnberg -> Leipzig -> A10 -> Berlin-Westhafen (52.54°N, 13.34°E)"
                    .into(),
            gps_waypoints: Vec::new(),
            google_maps_url: String::new(),
            recorded_temp_c: Some(-19.5),
            delay_minutes: 0,
            delivery_status: "Pünktlich & SLA-konform".into(),
        },
        FleetTourRecord {
            tour_id: "TOUR-2026-0408".into(),
            truck_id: "LKW-03".into(),
            license_plate: "DU-CH-3093".into(),
            driver_name: "Mehmet Yilmaz".into(),
            departure_date: "2026-04-08".into(),
            arrival_date: "2026-04-09".into(),
            month: "April 2026".into(),
            origin_warehouse_id: "WH-DUI-04".into(),
            origin_city: "Duisburg".into(),
            destination_warehouse_id: "WH-FRA-03".into(),
            destination_city: "Frankfurt".into(),
            customer_name: "ChemCorp Rhein-Main AG".into(),
            cargo_description: "Reinraum-Chemikalien & Elektrolyte (ADR Klasse 3)".into(),
            cargo_category: "ADR-Gefahrgut".into(),
            cargo_weight_tons: 20.5,
            distance_km: 235,
            gps_route: "Duisburg-Ruhrort -> A3 -> Köln -> Limburg -> Frankfurt-Süd".into(),
            gps_waypoints: Vec::new(),
            google_maps_url: String::new(),
            recorded_temp_c: None,
            delay_minutes: 0,
            delivery_status: "Pünktlich & ADR-konform".into(),
        },
        FleetTourRecord {
            tour_id: "TOUR-2026-0415".into(),
            truck_id: "LKW-07".into(),
            license_plate: "HH-LG-2077".into(),
            driver_name: "Murat Demir".into(),
            departure_date: "2026-04-16".into(),
            arrival_date: "2026-04-17".into(),
            month: "April 2026".into(),
            origin_warehouse_id: "WH-MUC-02".into(),
            origin_city: "München".into(),
            destination_warehouse_id: "WH-HAM-01".into(),
            destination_city: "Hamburg".into(),
            customer_name: "Airbus Operations Hamburg".into(),
            cargo_description: "Schwerlast-Turbinenträger & Titanbauteile".into(),
            cargo_category: "Schwerlast".into(),
            cargo_weight_tons: 29.8,
            distance_km: 775,
            gps_route: "München-Nord -> A9 -> A7 -> Kassel -> Hannover -> Hamburg-Hafen".into(),
            gps_waypoints: Vec::new(),
            google_maps_url: String::new(),
            recorded_temp_c: None,
            delay_minutes: 95,
            delivery_status:
                "Verspätet (+95 Min) - Danach LKW-07 wegen Ölstand 34% & Inspektion (180 km Rest) gesperrt!"
                    .into(),
        },
        FleetTourRecord {
            tour_id: "TOUR-2026-0421".into(),
            truck_id: "LKW-11".into(),
            license_plate: "F-FR-1121".into(),
            driver_name: "Marco Rossi".into(),
            departure_date: "2026-04-21".into(),
            arrival_date: "2026-04-22".into(),
            month: "April 2026".into(),
            origin_warehouse_id: "WH-FRA-03".into(),
            origin_city: "Frankfurt".into(),
            destination_warehouse_id: "WH-MUC-02".into(),
            destination_city: "München".into(),
            customer_name: "BMW Group Werk München".into(),
            cargo_description: "Steuergeräte & Cockpit-Elektronik".into(),
            cargo_category: "Industrie & Automotive".into(),
            cargo_weight_tons: 16.0,
            distance_km: 392,
            gps_route: "Frankfurt-Süd -> A3 -> Nürnberg -> A9 -> München-Nord".into(),
            gps_waypoints: Vec::new(),
            google_maps_url: String::new(),
            recorded_temp_c: None,
            delay_minutes: 0,
            delivery_status: "Pünktlich & SLA-konform".into(),
        },
        FleetTourRecord {
            tour_id: "TOUR-2026-0426".into(),
            truck_id: "LKW-12".into(),
            license_plate: "M-SL-1222".into(),
            driver_name: "Florian Bichler".into(),
            departure_date: "2026-04-26".into(),
            arrival_date: "2026-04-27".into(),
            month: "April 2026".into(),
            origin_warehouse_id: "WH-DUI-04".into(),
            origin_city: "Duisburg".into(),
            destination_warehouse_id: "WH-MUC-02".into(),
            destination_city: "München".into(),
            customer_name: "MAN Truck & Bus München".into(),
            cargo_description: "Schwerlast-Achsmodule & Getriebeeinheiten".into(),
            cargo_category: "Schwerlast".into(),
            cargo_weight_tons: 28.4,
            distance_km: 615,
            gps_route: "Duisburg-Ruhrort -> A3 -> Frankfurt -> Würzburg -> A9 -> München-Nord"
                .into(),
            gps_waypoints: Vec::new(),
            google_maps_url: String::new(),
            recorded_temp_c: None,
            delay_minutes: 25,
            delivery_status:
                "Geliefert - Danach LKW-12 wegen Inspektionslimit (450 km Rest) gesperrt".into(),
        },
    ];

    if truck_count > 12 {
        let extra_corridors = [
            (
                "WH-HAM-01",
                "Hamburg",
                "WH-MUC-02",
                "München",
                "BMW Group Regional-Shuttle",
                "Automotive-Komponenten (Shuttle)",
                "Regional-Shuttle",
                145,
                "Regional-Shuttle Nord/Süd",
            ),
            (
                "WH-FRA-03",
                "Frankfurt",
                "WH-DUI-04",
                "Duisburg",
                "ChemCorp Rhein-Main Shuttle",
                "Labor-Zubehör",
                "Regional-Shuttle",
                120,
                "Frankfurt -> A3 -> Rhein-Ruhr",
            ),
            (
                "WH-MUC-02",
                "München",
                "WH-FRA-03",
                "Frankfurt",
                "Sanofi Regional-Depot",
                "Medizinprodukte (-20°C)",
                "Pharma-Kühlkette",
                160,
                "Süd-Shuttle A9/A3",
            ),
            (
                "WH-DUI-04",
                "Duisburg",
                "WH-HAM-01",
                "Hamburg",
                "Hapag-LloydZubringer",
                "Container-Zubringer",
                "Regional-Shuttle",
                110,
                "West-Nord-Zubringer A1",
            ),
        ];
        for idx in 13..=truck_count {
            let v = &vehicles[idx - 1];
            let (owh, ocity, dwh, dcity, cust, cdesc, ccat, dist, gps) =
                extra_corridors[idx % extra_corridors.len()];
            tours.push(FleetTourRecord {
                tour_id: format!("TOUR-2026-03{:03}", idx + 30),
                truck_id: v.truck_id.clone(),
                license_plate: v.license_plate.clone(),
                driver_name: v.assigned_driver.clone(),
                departure_date: format!("2026-03-{:02}", (idx % 27) + 1),
                arrival_date: format!("2026-03-{:02}", (idx % 27) + 2),
                month: "März 2026".into(),
                origin_warehouse_id: owh.into(),
                origin_city: ocity.into(),
                destination_warehouse_id: dwh.into(),
                destination_city: dcity.into(),
                customer_name: cust.into(),
                cargo_description: cdesc.into(),
                cargo_category: ccat.into(),
                cargo_weight_tons: 18.0,
                distance_km: dist,
                gps_route: gps.into(),
                gps_waypoints: Vec::new(),
                google_maps_url: String::new(),
                recorded_temp_c: if ccat == "Pharma-Kühlkette" {
                    Some(-20.0)
                } else {
                    None
                },
                delay_minutes: 0,
                delivery_status: "Pünktlich & SLA-konform".into(),
            });
            tours.push(FleetTourRecord {
                tour_id: format!("TOUR-2026-04{:03}", idx + 40),
                truck_id: v.truck_id.clone(),
                license_plate: v.license_plate.clone(),
                driver_name: v.assigned_driver.clone(),
                departure_date: format!("2026-04-{:02}", (idx % 27) + 1),
                arrival_date: format!("2026-04-{:02}", (idx % 27) + 2),
                month: "April 2026".into(),
                origin_warehouse_id: owh.into(),
                origin_city: ocity.into(),
                destination_warehouse_id: dwh.into(),
                destination_city: dcity.into(),
                customer_name: cust.into(),
                cargo_description: cdesc.into(),
                cargo_category: ccat.into(),
                cargo_weight_tons: 19.5,
                distance_km: dist,
                gps_route: gps.into(),
                gps_waypoints: Vec::new(),
                google_maps_url: String::new(),
                recorded_temp_c: if ccat == "Pharma-Kühlkette" {
                    Some(-19.8)
                } else {
                    None
                },
                delay_minutes: 0,
                delivery_status: "Pünktlich & SLA-konform".into(),
            });
        }
    }

    for t in &mut tours {
        populate_tour_gps_waypoints(t);
    }

    (vehicles, tours, warehouses)
}

/// Generates a complete, unbroken **601-point (`1 Hz`) GPS stream** (`08:00:00` to `08:10:00`, every single second!)
/// distributed **evenly across the entire 775 km route from München (`WH-MUC-02`) to Hamburg (`WH-HAM-01`)**
/// for `TOUR-2026-0415` (`LKW-07`, `HH-LG-2077`).
fn generate_1hz_elbtunnel_tour_waypoints(d: &str) -> Vec<GpsWaypoint> {
    let mut out = Vec::with_capacity(601);

    let road_keyframes: &[(usize, f64, f64, &str)] = &[
        (
            0,
            48.2489,
            11.6214,
            "Start: Logistik-Hub München-Nord (WH-MUC-02)",
        ),
        (30, 48.5450, 11.5150, "A9 Pfaffenhofen / Holledau (Bayern)"),
        (60, 48.7665, 11.4257, "A9 Ingolstadt-Nord / Donau"),
        (90, 49.1150, 11.2850, "A9 Hilpoltstein / Mittelfranken"),
        (120, 49.4120, 11.1680, "A9/A73 Autobahnkreuz Nürnberg"),
        (150, 49.8980, 10.9020, "A73 Bamberg / Oberfranken"),
        (180, 50.2650, 10.9450, "A73 Coburg / Südthüringen"),
        (
            195,
            50.6050,
            10.7150,
            "A71 Suhl / Einfahrt Südportal Rennsteigtunnel",
        ),
        (
            225,
            50.6780,
            10.7620,
            "A71 Rennsteigtunnel (7.9 km Tunnel unter dem Thüringer Wald)",
        ),
        (
            255,
            50.7520,
            10.8120,
            "A71 Ausfahrt Nordportal Rennsteigtunnel / Ilmenau",
        ),
        (285, 50.9480, 10.3150, "A4 Eisenach / Hörselberge"),
        (
            315,
            50.8782,
            9.7024,
            "A7 Kirchheimer Dreieck / Bad Hersfeld",
        ),
        (
            340,
            51.1450,
            9.5350,
            "A7 Homberg (Efze) / Südliche Kasseler Berge",
        ),
        (
            365,
            51.3127,
            9.4797,
            "A7 Kasseler Berge (Unwetter- & Gewitterfront)",
        ),
        (390, 51.5350, 9.9050, "A7 Göttingen / Leinetal"),
        (430, 51.9650, 9.8450, "A7 Hildesheimer Börde"),
        (470, 52.4125, 9.7350, "A7/A2 Autobahnkreuz Hannover-Ost"),
        (510, 52.8950, 9.6950, "A7 Soltau / Lüneburger Heide"),
        (540, 53.3850, 9.9280, "A7 Maschener Kreuz / Seevetal"),
        (
            560,
            53.4980,
            9.9230,
            "A7 Hochstraße Elbmarsch / Elbbrücke Hamburg",
        ),
        (
            572,
            53.5360,
            9.9212,
            "A7 Einfahrt Hamburger Elbtunnel Südportal",
        ),
        (
            586,
            53.5445,
            9.9420,
            "A7 Hamburger Elbtunnel (42m unter der Norderelbe)",
        ),
        (
            600,
            53.5413,
            9.9841,
            "Ziel: Zentrallager Hamburg-Hafen (WH-HAM-01)",
        ),
    ];

    let interp_road = |s: usize| -> (f64, f64, &str) {
        for win in road_keyframes.windows(2) {
            let (s0, lat0, lon0, name0) = win[0];
            let (s1, lat1, lon1, name1) = win[1];
            if s >= s0 && s <= s1 {
                let frac = if s1 > s0 {
                    (s - s0) as f64 / (s1 - s0) as f64
                } else {
                    0.0
                };
                let sec_name = if frac < 0.5 { name0 } else { name1 };
                return (
                    lat0 + (lat1 - lat0) * frac,
                    lon0 + (lon1 - lon0) * frac,
                    sec_name,
                );
            }
        }
        (
            53.5413,
            9.9841,
            "Ziel: Zentrallager Hamburg-Hafen (WH-HAM-01)",
        )
    };

    for sec in 0..=600usize {
        let total_s = 8 * 3600 + sec;
        let hh = total_s / 3600;
        let mm = (total_s % 3600) / 60;
        let ss = total_s % 60;
        let ts_str = format!("{} {:02}:{:02}:{:02}", d, hh, mm, ss);

        let (base_lat, base_lon, section_label) = interp_road(sec);
        let mut lat = base_lat;
        let mut lon = base_lon;

        let (loc_name, spd, sats, hdop, acc_m, fix_type, weather, amb_c, rain_mm, wind_kmh) =
            if (195..=255).contains(&sec) {
                let t_idx = sec - 195;
                if t_idx < 8 {
                    let sign = if sec % 2 == 0 { 1.0 } else { -1.0 };
                    lon += sign * 0.004;
                    (
                        format!(
                            "A71 Rennsteigtunnel Südportal (1-Hz #{:03}: GNSS-Abfall)",
                            sec + 1
                        ),
                        76,
                        3 - (t_idx as i64 / 4),
                        6.4,
                        42.0 + (t_idx as f64) * 3.5,
                        "2D-Degraded (Tunnelportal < 4 Sat.)",
                        "⛅ Bewölkt (Thüringer Wald)",
                        9.5,
                        1.5,
                        28,
                    )
                } else if t_idx <= 52 {
                    let p = (t_idx - 8) as f64 / 44.0;
                    let arc = (p * std::f64::consts::PI).sin();
                    lon += 0.018 * arc;
                    lat -= 0.006 * arc;
                    let ac = ((58.0 + p * 60.5) * 10.0).round() / 10.0;
                    (
                        format!(
                            "A71 Rennsteigtunnel (1-Hz #{:03}: 0 Sat. Tunnel-Blackout · Drift ±{:.1}m)",
                            sec + 1,
                            ac
                        ),
                        78,
                        0,
                        99.9,
                        ac,
                        "No-Fix / IMU Dead-Reckoning (0 Sat.)",
                        "🚇 Tunnel (Rennsteigtunnel / Kein Wetter)",
                        15.5,
                        0.0,
                        0,
                    )
                } else {
                    (
                        format!(
                            "A71 Rennsteigtunnel Nordportal (1-Hz #{:03}: Satelliten Re-Lock)",
                            sec + 1
                        ),
                        79,
                        3 + ((t_idx - 52) as i64),
                        3.8,
                        28.0 - ((t_idx - 52) as f64) * 2.5,
                        "2D/3D Re-Akquise (Nordportal)",
                        "⛅ Leicht bewölkt",
                        10.2,
                        0.0,
                        22,
                    )
                }
            } else if (340..=385).contains(&sec) {
                let k_idx = sec - 340;
                let sign = if sec % 2 == 0 { 1.0 } else { -1.0 };
                let jitter = ((k_idx as f64 / 45.0) * std::f64::consts::PI).sin();
                lon += sign * 0.012 * jitter;
                lat += sign * 0.005 * jitter;
                let s_cnt = if (10..=35).contains(&k_idx) {
                    2 + (sec % 2) as i64
                } else {
                    4
                };
                let hd = if s_cnt < 4 { 7.8 } else { 3.2 };
                let ac = ((35.0 + 33.5 * jitter) * 10.0).round() / 10.0;
                (
                    format!(
                        "A7 Kasseler Berge (1-Hz #{:03}: Gewitter & Starkregen · {} Sat. ±{:.1}m)",
                        sec + 1,
                        s_cnt,
                        ac
                    ),
                    64,
                    s_cnt,
                    hd,
                    ac,
                    if s_cnt < 4 {
                        "2D-Degraded (< 4 Sat. durch Starkregen!)"
                    } else {
                        "3D-Fix (Gedämpft)"
                    },
                    "⛈️ Schweres Gewitter & Starkregen",
                    8.6,
                    ((38.0 + 14.0 * jitter) * 10.0).round() / 10.0,
                    74,
                )
            } else if (555..=595).contains(&sec) {
                if sec < 570 {
                    (
                        format!(
                            "A7 Hochstraße Elbmarsch / Elbbrücke (1-Hz #{:03}: Orkanböen 86 km/h!)",
                            sec + 1
                        ),
                        58,
                        6,
                        2.1,
                        6.4,
                        "3D-Fix (Sturmböen)",
                        "💨 Orkanböen (Seitenwind) & Starkregen",
                        10.8,
                        33.4,
                        86,
                    )
                } else {
                    let e_idx = (sec - 570) as f64;
                    let arc = ((e_idx / 25.0) * std::f64::consts::PI).sin();
                    lat += 0.008 * arc;
                    lon -= 0.006 * arc;
                    let s_cnt = if (3..=22).contains(&(sec - 570)) {
                        0
                    } else {
                        3
                    };
                    let ac = ((54.0 + 70.7 * (e_idx / 25.0)) * 10.0).round() / 10.0;
                    (
                        format!(
                            "A7 Hamburger Elbtunnel (1-Hz #{:03}: {} Sat. Unterwasser-Röhre ±{:.1}m)",
                            sec + 1,
                            s_cnt,
                            ac
                        ),
                        58,
                        s_cnt,
                        if s_cnt == 0 { 99.9 } else { 5.8 },
                        ac,
                        if s_cnt == 0 {
                            "No-Fix / IMU Dead-Reckoning (0 Sat.)"
                        } else {
                            "2D-Degraded (3 Sat. Tunnelportal)"
                        },
                        "🚇 Tunnel (42m unter der Elbe / Kein Wetter)",
                        16.4,
                        0.0,
                        0,
                    )
                }
            } else {
                let sp = if sec == 0 || sec == 600 {
                    0
                } else {
                    82 + ((sec % 5) as i64) - 2
                };
                let s_cnt = 11 + ((sec % 4) as i64);
                let hd = 0.8 + ((sec % 3) as f64) * 0.1;
                let ac = 1.5 + ((sec % 4) as f64) * 0.1;
                let (wth, amb, rn, wnd) = if sec < 150 {
                    ("☀️ Sonnig & Trocken", 13.5, 0.0, 14)
                } else if sec < 330 {
                    ("⛅ Leicht bewölkt", 12.0, 0.0, 22)
                } else if sec < 540 {
                    ("🌦️ Wechselhaft & Windig", 11.5, 4.5, 38)
                } else {
                    ("🌧️ Küstenregen Hamburg", 11.2, 12.0, 46)
                };
                (
                    format!("{} (1-Hz #{:03})", section_label, sec + 1),
                    sp,
                    s_cnt,
                    hd,
                    (ac * 10.0).round() / 10.0,
                    "3D-Fix",
                    wth,
                    amb,
                    rn,
                    wnd,
                )
            };

        out.push(GpsWaypoint {
            timestamp: ts_str,
            lat: (lat * 100000.0).round() / 100000.0,
            lon: (lon * 100000.0).round() / 100000.0,
            location_name: loc_name,
            speed_kmh: spd,
            temp_c: None,
            satellites_used: sats,
            hdop,
            gps_accuracy_m: acc_m,
            fix_type: fix_type.to_string(),
            weather_condition: weather.to_string(),
            ambient_temp_c: amb_c,
            precipitation_mm_h: rain_mm,
            wind_speed_kmh: wind_kmh,
        });
    }

    out
}

/// Populates realistic GPS telemetry waypoints including 1-Hz sequences, GNSS satellite metrics,
/// live weather telemetry, and Google Maps URLs for a tour.
fn populate_tour_gps_waypoints(t: &mut FleetTourRecord) {
    let d = &t.departure_date;
    let base_temp = t.recorded_temp_c;

    if t.tour_id == "TOUR-2026-0415" {
        let waypoints = generate_1hz_elbtunnel_tour_waypoints(d);
        t.google_maps_url = "https://www.google.com/maps/dir/48.2489,11.6214/49.7660,9.9530/51.3127,9.4797/52.4125,9.7350/53.4685,9.9175/53.5396,9.9212/53.5518,9.9188/53.5413,9.9841".to_string();
        t.gps_waypoints = waypoints;
        return;
    }

    type RawWp<'a> = (
        &'a str,
        f64,
        f64,
        &'a str,
        i64,
        Option<f64>,
        i64,
        f64,
        f64,
        &'a str,
        &'a str,
        f64,
        f64,
        i64,
    );

    let raw_points: Vec<RawWp> = if t.tour_id == "TOUR-2026-0412" {
        vec![
            ("05:30:00", 50.0264, 8.5583, "Start: Pharma-Kühllager Frankfurt-Süd (WH-FRA-03)", 0, Some(-19.6), 13, 0.8, 1.5, "3D-Fix", "☀️ Klar & Trocken", 11.5, 0.0, 14),
            ("07:55:01", 51.3120, 9.4790, "A7 Kasseler Berge (1-Hz: Einfahrt Gewitterzelle)", 78, Some(-18.6), 6, 2.1, 6.8, "3D-Fix", "🌧️ Starkregen", 9.2, 28.5, 54),
            ("07:55:02", 51.3129, 9.4804, "A7 Kasseler Berge (1-Hz: Wolkenbruch & Signal-Dämpfung)", 74, Some(-18.5), 3, 5.8, 42.0, "2D-Degraded (3 Sat.)", "⛈️ Schweres Gewitter & Starkregen", 8.8, 46.0, 68),
            ("07:55:03", 51.3142, 9.4828, "A7 Kasseler Berge (1-Hz: Kritischer GNSS-Verlust 2 Sat. / 68.5m Spur-Drift!)", 68, Some(-18.4), 2, 7.9, 68.5, "2D-Degraded (2 Sat.)", "⛈️ Schweres Gewitter & Starkregen", 8.5, 52.0, 74),
            ("07:55:04", 51.3118, 9.4775, "A7 Kasseler Berge (1-Hz: GPS-Ausreißer 64m westlich der A7)", 65, Some(-18.3), 2, 7.4, 64.0, "2D-Degraded (2 Sat.)", "⛈️ Schweres Gewitter & Starkregen", 8.6, 49.5, 71),
            ("07:55:05", 51.3135, 9.4812, "A7 Kasseler Berge (1-Hz: Teil-Rekonstruktion 3 Satelliten)", 66, Some(-18.2), 3, 5.1, 38.5, "2D-Degraded (3 Sat.)", "⛈️ Starkregen & Sturmböen", 8.9, 39.0, 62),
            ("07:55:06", 51.3138, 9.4801, "A7 Kasseler Berge (1-Hz: 3D-Fix wiederhergestellt)", 72, Some(-18.2), 9, 1.3, 3.2, "3D-Fix", "🌧️ Abziehender Regen", 10.1, 12.0, 38),
            ("10:15:00", 52.2689, 10.5268, "A2 Braunschweig-Nord (Thermostatventil-Warnung!)", 62, Some(-15.8), 12, 0.9, 1.8, "3D-Fix", "⛅ Wechselhaft", 19.5, 0.0, 22),
            ("12:10:01", 52.1205, 11.6276, "A2 Magdeburg-Zentrum (1-Hz: Stau & Hitzestau am Kühlkoffer)", 14, Some(-14.4), 14, 0.7, 1.4, "3D-Fix", "☀️ Sonnig & Heiß (Hitzestau)", 26.4, 0.0, 8),
            ("12:10:02", 52.1206, 11.6279, "A2 Magdeburg-Zentrum (1-Hz: SLA-VERSTOSS -14.2°C erreicht!)", 8, Some(-14.2), 14, 0.7, 1.4, "3D-Fix", "☀️ Sonnig & Heiß (Hitzestau)", 26.5, 0.0, 7),
            ("12:10:03", 52.1207, 11.6281, "A2 Magdeburg-Zentrum (1-Hz: Stillstand & Kompressor-Alarm)", 0, Some(-14.2), 13, 0.8, 1.5, "3D-Fix", "☀️ Sonnig & Heiß (Hitzestau)", 26.5, 0.0, 8),
            ("14:35:00", 52.5364, 13.3389, "Ziel: City-Hub Berlin-Westhafen (WH-BER-05 - Quarantäne-Prüfung)", 0, Some(-14.9), 12, 0.9, 1.7, "3D-Fix", "⛅ Leicht bewölkt", 21.0, 0.0, 18),
        ]
    } else if t.tour_id == "TOUR-2026-0405" {
        vec![
            ("06:15:00", 53.5413, 9.9841, "Start: Zentrallager Hamburg-Hafen (WH-HAM-01)", 0, Some(-18.4), 12, 0.9, 1.7, "3D-Fix", "🌫️ Küstennebel", 7.2, 0.5, 22),
            ("07:30:00", 53.4650, 11.1550, "A24 Raststätte Vellahn (Mecklenburg)", 85, Some(-18.4), 13, 0.8, 1.5, "3D-Fix", "⛅ Bewölkt", 8.5, 0.0, 19),
            ("08:45:00", 53.1636, 12.4851, "A24/A19 Autobahndreieck Wittstock/Dosse", 83, Some(-18.4), 12, 0.9, 1.6, "3D-Fix", "🌫️ Dichter Nebel (Sicht < 50m)", 6.5, 1.5, 10),
            ("09:35:00", 52.9275, 12.8058, "A24 Neuruppin / Fehrbellin (Brandenburg)", 76, Some(-18.4), 11, 1.0, 1.9, "3D-Fix", "🌧️ Einsetzender Regen", 8.9, 9.5, 28),
            ("10:30:01", 52.5361, 13.3382, "Berlin-Westhafen Einfahrt (1-Hz: Starkregen-Schauer)", 22, Some(-18.4), 8, 1.5, 3.8, "3D-Fix", "🌧️ Starkregen-Schauer", 9.5, 22.0, 35),
            ("10:30:02", 52.5372, 13.3401, "Berlin-Westhafen Containerkran-Gasse (1-Hz: Multipath + Starkregen 3 Sat.!)", 12, Some(-18.4), 3, 5.5, 44.0, "2D-Degraded (3 Sat.)", "🌧️ Starkregen-Schauer", 9.4, 25.0, 38),
            ("10:30:03", 52.5355, 13.3374, "Berlin-Westhafen Stahlbrücke (1-Hz: Multipath-Sprung ±48.5m)", 8, Some(-18.4), 3, 5.9, 48.5, "2D-Degraded (3 Sat.)", "🌧️ Starkregen-Schauer", 9.4, 24.0, 36),
            ("10:30:04", 52.5364, 13.3389, "Ziel: City-Hub Berlin-Westhafen (WH-BER-05 - 3D-Fix wieder stabil)", 0, Some(-18.4), 11, 0.9, 1.9, "3D-Fix", "🌦️ Abklingender Schauer", 9.8, 4.5, 20),
        ]
    } else {
        match (t.origin_warehouse_id.as_str(), t.destination_warehouse_id.as_str()) {
            ("WH-FRA-03", "WH-BER-05") => vec![
                ("06:00:01", 50.0264, 8.5583, "Start: Pharma-Kühllager Frankfurt-Süd (WH-FRA-03)", 0, base_temp, 13, 0.8, 1.5, "3D-Fix", "☀️ Sonnig & Trocken", 12.5, 0.0, 14),
                ("08:05:01", 50.8782, 9.7024, "A5/A4 Kirchheimer Dreieck (Hessen)", 84, base_temp, 12, 0.9, 1.7, "3D-Fix", "⛅ Leicht bewölkt", 14.2, 0.0, 18),
                ("09:50:01", 50.9787, 11.0328, "A4/A71 Autobahnkreuz Erfurt (Thüringen)", 82, base_temp, 14, 0.7, 1.4, "3D-Fix", "☀️ Sonnig & Trocken", 16.0, 0.0, 15),
                ("11:30:01", 51.4239, 12.2225, "A9 Schkeuditzer Kreuz (Leipzig / Halle)", 86, base_temp, 13, 0.8, 1.5, "3D-Fix", "☀️ Sonnig & Trocken", 17.5, 0.0, 16),
                ("13:20:01", 52.5364, 13.3389, "Ziel: City-Hub Berlin-Westhafen (WH-BER-05)", 0, base_temp, 12, 0.9, 1.6, "3D-Fix", "⛅ Leicht bewölkt", 18.0, 0.0, 19),
            ],
            ("WH-HAM-01", "WH-BER-05") => vec![
                ("06:15:01", 53.5413, 9.9841, "Start: Zentrallager Hamburg-Hafen (WH-HAM-01)", 0, base_temp, 12, 0.9, 1.7, "3D-Fix", "⛅ Küstenbewölkung", 11.0, 0.0, 24),
                ("07:30:01", 53.4650, 11.1550, "A24 Raststätte Vellahn (Mecklenburg)", 85, base_temp, 13, 0.8, 1.5, "3D-Fix", "☀️ Sonnig & Trocken", 13.5, 0.0, 20),
                ("08:45:01", 53.1636, 12.4851, "A24/A19 Autobahndreieck Wittstock/Dosse", 83, base_temp, 14, 0.7, 1.4, "3D-Fix", "☀️ Sonnig & Trocken", 15.0, 0.0, 16),
                ("09:35:01", 52.9275, 12.8058, "A24 Neuruppin / Fehrbellin (Brandenburg)", 80, base_temp, 13, 0.8, 1.5, "3D-Fix", "☀️ Sonnig & Trocken", 16.2, 0.0, 14),
                ("10:30:01", 52.5364, 13.3389, "Ziel: City-Hub Berlin-Westhafen (WH-BER-05)", 0, base_temp, 12, 0.9, 1.6, "3D-Fix", "☀️ Sonnig & Trocken", 17.0, 0.0, 15),
            ],
            ("WH-DUI-04", "WH-BER-05") => vec![
                ("05:00:01", 51.4566, 6.7422, "Start: Rhein-Ruhr Gefahrgut-Depot Duisburg (WH-DUI-04)", 0, base_temp, 12, 0.9, 1.7, "3D-Fix", "⛅ Leicht bewölkt", 12.0, 0.0, 18),
                ("06:50:01", 51.8052, 8.0195, "A2 Raststätte Rhynern / Beckum (NRW)", 82, base_temp, 13, 0.8, 1.5, "3D-Fix", "🌦️ Leichter Schauer", 13.0, 2.5, 25),
                ("08:45:01", 52.4125, 9.7350, "A2/A7 Autobahnkreuz Hannover-Ost", 84, base_temp, 13, 0.8, 1.6, "3D-Fix", "⛅ Bewölkt & Trocken", 14.8, 0.0, 21),
                ("10:35:01", 52.1856, 11.6384, "A2/A14 Autobahnkreuz Magdeburg", 83, base_temp, 14, 0.7, 1.4, "3D-Fix", "☀️ Sonnig & Trocken", 16.5, 0.0, 16),
                ("12:15:01", 52.5364, 13.3389, "Ziel: City-Hub Berlin-Westhafen (WH-BER-05)", 0, base_temp, 12, 0.9, 1.7, "3D-Fix", "☀️ Sonnig & Trocken", 17.8, 0.0, 14),
            ],
            ("WH-MUC-02", "WH-BER-05") => vec![
                ("05:30:01", 48.2489, 11.6214, "Start: Logistik-Hub München-Nord (WH-MUC-02)", 0, base_temp, 13, 0.8, 1.5, "3D-Fix", "☀️ Klar & Trocken", 10.5, 0.0, 12),
                ("07:35:01", 49.4120, 11.1680, "A9/A6 Autobahnkreuz Nürnberg-Süd", 85, base_temp, 14, 0.7, 1.4, "3D-Fix", "☀️ Sonnig & Trocken", 13.8, 0.0, 14),
                ("09:20:01", 50.3210, 11.8320, "A9 Dreieck Bayerisches Vogtland / Hof", 81, base_temp, 11, 1.0, 1.9, "3D-Fix", "⛅ Bewölkt (Mittelgebirge)", 12.2, 0.5, 26),
                ("11:05:01", 51.3150, 12.1640, "A9 Kreuz Rippachtal / Leipzig-West", 86, base_temp, 13, 0.8, 1.5, "3D-Fix", "☀️ Sonnig & Trocken", 16.4, 0.0, 18),
                ("13:10:01", 52.5364, 13.3389, "Ziel: City-Hub Berlin-Westhafen (WH-BER-05)", 0, base_temp, 12, 0.9, 1.6, "3D-Fix", "☀️ Sonnig & Trocken", 18.2, 0.0, 15),
            ],
            ("WH-FRA-03", "WH-MUC-02") => vec![
                ("06:00:01", 50.0264, 8.5583, "Start: Pharma-Kühllager Frankfurt-Süd (WH-FRA-03)", 0, base_temp, 13, 0.8, 1.5, "3D-Fix", "☀️ Sonnig & Trocken", 13.0, 0.0, 12),
                ("07:30:01", 49.7660, 9.9530, "A3 Autobahnkreuz Biebelried / Würzburg", 84, base_temp, 12, 0.9, 1.7, "3D-Fix", "☀️ Sonnig & Trocken", 15.2, 0.0, 16),
                ("09:00:01", 49.4120, 11.1680, "A3/A9 Autobahnkreuz Nürnberg", 82, base_temp, 14, 0.7, 1.4, "3D-Fix", "⛅ Leicht bewölkt", 16.5, 0.0, 15),
                ("10:15:01", 48.7665, 11.4257, "A9 Ingolstadt-Nord", 85, base_temp, 13, 0.8, 1.5, "3D-Fix", "☀️ Sonnig & Trocken", 17.8, 0.0, 13),
                ("11:15:01", 48.2489, 11.6214, "Ziel: Logistik-Hub München-Nord (WH-MUC-02)", 0, base_temp, 13, 0.8, 1.5, "3D-Fix", "☀️ Sonnig & Trocken", 18.5, 0.0, 11),
            ],
            ("WH-MUC-02", "WH-FRA-03") => vec![
                ("06:30:01", 48.2489, 11.6214, "Start: Logistik-Hub München-Nord (WH-MUC-02)", 0, base_temp, 13, 0.8, 1.5, "3D-Fix", "☀️ Sonnig & Trocken", 11.5, 0.0, 10),
                ("07:35:01", 48.7665, 11.4257, "A9 Ingolstadt-Nord", 84, base_temp, 14, 0.7, 1.4, "3D-Fix", "☀️ Sonnig & Trocken", 13.5, 0.0, 14),
                ("08:50:01", 49.4120, 11.1680, "A9/A3 Autobahnkreuz Nürnberg", 83, base_temp, 12, 0.9, 1.6, "3D-Fix", "⛅ Leicht bewölkt", 15.0, 0.0, 17),
                ("10:20:01", 49.7660, 9.9530, "A3 Würzburg / Spessart", 82, base_temp, 11, 1.0, 1.9, "3D-Fix", "🌦️ Leichter Regen (Spessart)", 13.8, 3.2, 24),
                ("11:45:01", 50.0264, 8.5583, "Ziel: Pharma-Kühllager Frankfurt-Süd (WH-FRA-03)", 0, base_temp, 13, 0.8, 1.5, "3D-Fix", "⛅ Bewölkt & Trocken", 16.2, 0.0, 15),
            ],
            ("WH-DUI-04", "WH-HAM-01") => vec![
                ("05:45:01", 51.4566, 6.7422, "Start: Rhein-Ruhr Gefahrgut-Depot Duisburg (WH-DUI-04)", 0, base_temp, 12, 0.9, 1.6, "3D-Fix", "⛅ Leicht bewölkt", 12.4, 0.0, 19),
                ("07:10:01", 51.9607, 7.6261, "A1 Münster-Nord (Westfalen)", 83, base_temp, 13, 0.8, 1.5, "3D-Fix", "☀️ Sonnig & Trocken", 14.0, 0.0, 18),
                ("08:35:01", 52.2799, 8.0472, "A1/A30 Lotter Kreuz / Osnabrück", 84, base_temp, 13, 0.8, 1.5, "3D-Fix", "⛅ Bewölkt", 14.5, 0.0, 22),
                ("10:00:01", 53.0450, 8.8820, "A1 Bremer Kreuz", 81, base_temp, 12, 0.9, 1.7, "3D-Fix", "🌦️ Küstenschauer", 13.2, 4.0, 31),
                ("11:20:01", 53.5413, 9.9841, "Ziel: Zentrallager Hamburg-Hafen (WH-HAM-01)", 0, base_temp, 11, 1.0, 1.9, "3D-Fix", "⛅ Wechselhaft & Windig", 13.8, 0.5, 34),
            ],
            ("WH-HAM-01", "WH-DUI-04") => vec![
                ("06:00:01", 53.5413, 9.9841, "Start: Zentrallager Hamburg-Hafen (WH-HAM-01)", 0, base_temp, 12, 0.9, 1.7, "3D-Fix", "⛅ Küstenbewölkung", 11.5, 0.0, 26),
                ("07:20:01", 53.0450, 8.8820, "A1 Bremer Kreuz", 82, base_temp, 13, 0.8, 1.5, "3D-Fix", "☀️ Sonnig & Trocken", 13.4, 0.0, 21),
                ("08:45:01", 52.2799, 8.0472, "A1 Osnabrück-Nord", 80, base_temp, 13, 0.8, 1.5, "3D-Fix", "☀️ Sonnig & Trocken", 15.1, 0.0, 17),
                ("10:10:01", 51.9607, 7.6261, "A1/A43 Kreuz Münster-Süd", 78, base_temp, 14, 0.7, 1.4, "3D-Fix", "☀️ Sonnig & Trocken", 16.5, 0.0, 15),
                ("11:30:01", 51.4566, 6.7422, "Ziel: Rhein-Ruhr Gefahrgut-Depot Duisburg (WH-DUI-04)", 0, base_temp, 12, 0.9, 1.6, "3D-Fix", "☀️ Sonnig & Trocken", 17.4, 0.0, 14),
            ],
            ("WH-MUC-02", "WH-HAM-01") => vec![
                ("04:30:01", 48.2489, 11.6214, "Start: Logistik-Hub München-Nord (WH-MUC-02)", 0, base_temp, 13, 0.8, 1.5, "3D-Fix", "☀️ Klar & Trocken", 9.5, 0.0, 11),
                ("06:45:01", 49.7660, 9.9530, "A9/A3/A7 Kreuz Schweinfurt/Werneck", 83, base_temp, 13, 0.8, 1.5, "3D-Fix", "☀️ Sonnig & Trocken", 12.8, 0.0, 15),
                ("09:15:01", 51.3127, 9.4797, "A7 Autobahnkreuz Kassel-Mitte", 80, base_temp, 11, 1.0, 1.9, "3D-Fix", "⛅ Bewölkt", 14.0, 0.0, 24),
                ("11:40:01", 52.4125, 9.7350, "A7/A2 Autobahnkreuz Hannover-Ost", 82, base_temp, 12, 0.9, 1.7, "3D-Fix", "🌦️ Leichter Regen", 13.5, 3.5, 29),
                ("13:45:01", 53.5413, 9.9841, "Ziel: Zentrallager Hamburg-Hafen (WH-HAM-01)", 0, base_temp, 12, 0.9, 1.7, "3D-Fix", "⛅ Wechselhaft", 14.0, 0.0, 30),
            ],
            ("WH-HAM-01", "WH-MUC-02") => vec![
                ("04:30:01", 53.5413, 9.9841, "Start: Zentrallager Hamburg-Hafen (WH-HAM-01)", 0, base_temp, 12, 0.9, 1.7, "3D-Fix", "⛅ Küstenbewölkung", 10.8, 0.0, 25),
                ("06:30:01", 52.4125, 9.7350, "A7 Autobahnkreuz Hannover-Ost", 85, base_temp, 13, 0.8, 1.5, "3D-Fix", "☀️ Sonnig & Trocken", 13.2, 0.0, 19),
                ("08:50:01", 51.3127, 9.4797, "A7 Autobahnkreuz Kassel-Mitte", 83, base_temp, 12, 0.9, 1.7, "3D-Fix", "⛅ Leicht bewölkt", 14.8, 0.0, 21),
                ("11:15:01", 49.4120, 11.1680, "A9 Autobahnkreuz Nürnberg", 84, base_temp, 14, 0.7, 1.4, "3D-Fix", "☀️ Sonnig & Trocken", 17.5, 0.0, 14),
                ("13:15:01", 48.2489, 11.6214, "Ziel: Logistik-Hub München-Nord (WH-MUC-02)", 0, base_temp, 13, 0.8, 1.5, "3D-Fix", "☀️ Sonnig & Trocken", 19.0, 0.0, 12),
            ],
            ("WH-DUI-04", "WH-FRA-03") => vec![
                ("06:30:01", 51.4566, 6.7422, "Start: Rhein-Ruhr Gefahrgut-Depot Duisburg (WH-DUI-04)", 0, base_temp, 12, 0.9, 1.6, "3D-Fix", "☀️ Sonnig & Trocken", 12.5, 0.0, 15),
                ("07:35:01", 50.9375, 6.9603, "A3 Autobahnkreuz Köln-Ost", 81, base_temp, 13, 0.8, 1.5, "3D-Fix", "☀️ Sonnig & Trocken", 14.2, 0.0, 16),
                ("08:40:01", 50.4350, 7.8250, "A3 Montabaur / Westerwald", 83, base_temp, 11, 1.0, 1.9, "3D-Fix", "⛅ Bewölkt (Westerwald)", 13.0, 0.5, 24),
                ("09:20:01", 50.3880, 8.0620, "A3 Limburg an der Lahn", 84, base_temp, 13, 0.8, 1.5, "3D-Fix", "☀️ Sonnig & Trocken", 15.8, 0.0, 17),
                ("10:10:01", 50.0264, 8.5583, "Ziel: Pharma-Kühllager Frankfurt-Süd (WH-FRA-03)", 0, base_temp, 13, 0.8, 1.5, "3D-Fix", "☀️ Sonnig & Trocken", 17.2, 0.0, 14),
            ],
            ("WH-FRA-03", "WH-DUI-04") => vec![
                ("06:30:01", 50.0264, 8.5583, "Start: Pharma-Kühllager Frankfurt-Süd (WH-FRA-03)", 0, base_temp, 13, 0.8, 1.5, "3D-Fix", "☀️ Sonnig & Trocken", 13.0, 0.0, 14),
                ("07:20:01", 50.3880, 8.0620, "A3 Limburg an der Lahn", 84, base_temp, 13, 0.8, 1.5, "3D-Fix", "☀️ Sonnig & Trocken", 14.5, 0.0, 16),
                ("08:30:01", 50.9375, 6.9603, "A3 Autobahnkreuz Köln-Ost", 82, base_temp, 12, 0.9, 1.7, "3D-Fix", "⛅ Leicht bewölkt", 16.0, 0.0, 18),
                ("09:20:01", 51.2277, 6.7735, "A3 Kreuz Ratingen / Düsseldorf", 80, base_temp, 12, 0.9, 1.7, "3D-Fix", "⛅ Leicht bewölkt", 16.8, 0.0, 19),
                ("10:05:01", 51.4566, 6.7422, "Ziel: Rhein-Ruhr Gefahrgut-Depot Duisburg (WH-DUI-04)", 0, base_temp, 13, 0.8, 1.5, "3D-Fix", "☀️ Sonnig & Trocken", 17.5, 0.0, 15),
            ],
            _ => vec![
                ("05:00:01", 51.4566, 6.7422, "Start: Rhein-Ruhr Gefahrgut-Depot Duisburg (WH-DUI-04)", 0, base_temp, 12, 0.9, 1.6, "3D-Fix", "☀️ Sonnig & Trocken", 12.0, 0.0, 15),
                ("07:15:01", 50.0264, 8.5583, "A3 Frankfurter Kreuz", 83, base_temp, 13, 0.8, 1.5, "3D-Fix", "☀️ Sonnig & Trocken", 15.0, 0.0, 16),
                ("09:10:01", 49.7660, 9.9530, "A3 Autobahnkreuz Biebelried / Würzburg", 82, base_temp, 13, 0.8, 1.5, "3D-Fix", "⛅ Leicht bewölkt", 16.4, 0.0, 18),
                ("11:00:01", 49.4120, 11.1680, "A9 Autobahnkreuz Nürnberg", 84, base_temp, 14, 0.7, 1.4, "3D-Fix", "☀️ Sonnig & Trocken", 17.8, 0.0, 14),
                ("12:45:01", 48.2489, 11.6214, "Ziel: Logistik-Hub München-Nord (WH-MUC-02)", 0, base_temp, 13, 0.8, 1.5, "3D-Fix", "☀️ Sonnig & Trocken", 19.0, 0.0, 12),
            ],
        }
    };

    let mut waypoints = Vec::with_capacity(raw_points.len());
    let mut coord_segments: Vec<String> = Vec::new();
    let mut last_gmap_coord: Option<(f64, f64)> = None;

    for (
        time_str,
        lat,
        lon,
        loc_name,
        spd,
        tmp,
        sats,
        hdop,
        acc_m,
        fix_t,
        weather,
        amb_c,
        rain_mm,
        wind_kmh,
    ) in raw_points
    {
        waypoints.push(GpsWaypoint {
            timestamp: format!("{} {}", d, time_str),
            lat,
            lon,
            location_name: loc_name.to_string(),
            speed_kmh: spd,
            temp_c: tmp,
            satellites_used: sats,
            hdop,
            gps_accuracy_m: acc_m,
            fix_type: fix_t.to_string(),
            weather_condition: weather.to_string(),
            ambient_temp_c: amb_c,
            precipitation_mm_h: rain_mm,
            wind_speed_kmh: wind_kmh,
        });

        let include_in_gmap = match last_gmap_coord {
            None => true,
            Some((plat, plon)) => (lat - plat).abs() + (lon - plon).abs() > 0.08,
        };
        if include_in_gmap {
            coord_segments.push(format!("{:.4},{:.4}", lat, lon));
            last_gmap_coord = Some((lat, lon));
        }
    }
    if let Some(last_wp) = waypoints.last() {
        let end_str = format!("{:.4},{:.4}", last_wp.lat, last_wp.lon);
        if coord_segments.last() != Some(&end_str) {
            coord_segments.push(end_str);
        }
    }

    t.google_maps_url = format!(
        "https://www.google.com/maps/dir/{}",
        coord_segments.join("/")
    );
    t.gps_waypoints = waypoints;
}
