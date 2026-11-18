use crate::AppState;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};

/// Renders an interactive OpenStreetMap (Leaflet.js) Route, 1-Hz GNSS Accuracy & Live Weather Map (`GET /v1/fleet/tours/:tour_id/map`)
pub async fn get_fleet_tour_map_handler(
    State(state): State<AppState>,
    Path(tour_id): Path<String>,
) -> impl IntoResponse {
    let Ok(Some(tour)) = state.db.get_fleet_tour(&tour_id).await else {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from(format!(
                "Tour '{}' not found in SurrealDB.",
                tour_id
            )))
            .unwrap();
    };

    let waypoints_json = serde_json::to_string(&tour.gps_waypoints).unwrap_or_else(|_| "[]".into());
    let has_gnss_anomaly = tour
        .gps_waypoints
        .iter()
        .any(|w| w.satellites_used < 4 || w.gps_accuracy_m >= 15.0);
    let is_alert = tour.delivery_status.to_lowercase().contains("verstoss")
        || tour.delivery_status.to_lowercase().contains("verspätet")
        || has_gnss_anomaly;
    let line_color = if is_alert { "#ef4444" } else { "#10b981" };

    let min_sats = tour
        .gps_waypoints
        .iter()
        .map(|w| w.satellites_used)
        .min()
        .unwrap_or(12);
    let max_acc_m = tour
        .gps_waypoints
        .iter()
        .map(|w| w.gps_accuracy_m)
        .fold(1.5_f64, f64::max);
    let max_rain = tour
        .gps_waypoints
        .iter()
        .map(|w| w.precipitation_mm_h)
        .fold(0.0_f64, f64::max);
    let max_wind = tour
        .gps_waypoints
        .iter()
        .map(|w| w.wind_speed_kmh)
        .max()
        .unwrap_or(15);

    let first_anomaly_idx = tour.gps_waypoints.iter().position(|w| {
        w.satellites_used < 4 || w.gps_accuracy_m >= 15.0 || w.precipitation_mm_h >= 20.0
    });

    let wp_count = tour.gps_waypoints.len();
    let low_sat_count = tour
        .gps_waypoints
        .iter()
        .filter(|w| w.satellites_used < 4)
        .count();

    let anomaly_zoom_btn = if wp_count > 50 {
        r#"<div style="display:flex; flex-wrap:wrap; gap:0.35rem; margin-top:0.45rem;">
            <button type="button" class="anomaly-btn" style="margin-top:0; flex:1; background:rgba(56,189,248,0.18); border-color:#38bdf8; color:#bae6fd;" onclick="fitFullRoute()">
                🗺️ Ganze Route
            </button>
            <button type="button" class="anomaly-btn" style="margin-top:0; flex:1;" onclick="focusAnomalyZone(220)">
                🚇 Rennsteigtunnel (#221)
            </button>
            <button type="button" class="anomaly-btn" style="margin-top:0; flex:1;" onclick="focusAnomalyZone(362)">
                ⛈️ Kasseler Berge (#363)
            </button>
            <button type="button" class="anomaly-btn" style="margin-top:0; flex:1;" onclick="focusAnomalyZone(582)">
                🚇 Elbtunnel HH (#583)
            </button>
            <button type="button" id="replayBtn" class="replay-btn" style="flex:1;" onclick="toggleReplay()">
                ▶️ 1-Hz Replay
            </button>
        </div>"#
            .to_string()
    } else if let Some(a_idx) = first_anomaly_idx {
        format!(
            r#"<div style="display:flex; gap:0.4rem; margin-top:0.45rem;">
                <button type="button" class="anomaly-btn" style="margin-top:0; flex:1;" onclick="focusAnomalyZone({a_idx})">
                    ⚠️ 1-Hz Störungs-/Tunnel-Zone (#{num})
                </button>
                <button type="button" id="replayBtn" class="replay-btn" onclick="toggleReplay()">
                    ▶️ 1-Hz Replay
                </button>
            </div>"#,
            a_idx = a_idx,
            num = a_idx + 1
        )
    } else {
        String::new()
    };

    let mut wp_cards_html = String::new();
    for (idx, wp) in tour.gps_waypoints.iter().enumerate() {
        let is_bad_gnss = wp.satellites_used < 4 || wp.gps_accuracy_m >= 15.0;
        let icon = if idx == 0 {
            "🟢 Start"
        } else if idx + 1 == tour.gps_waypoints.len() {
            "🏁 Ziel"
        } else if wp.satellites_used == 0 {
            "🚇 0 Sat. Tunnel"
        } else if is_bad_gnss {
            "⚠️ 1-Hz GNSS-Drift"
        } else {
            "🛰️ 1-Hz GPS"
        };
        let temp_txt = match wp.temp_c {
            Some(tc) => format!(" · ❄️ Kühlraum: <b>{:.1} °C</b>", tc),
            None => String::new(),
        };
        let card_cls = if is_bad_gnss {
            "wp-card wp-card-alert"
        } else {
            "wp-card"
        };
        let gnss_badge_cls = if is_bad_gnss {
            "gnss-pill alert"
        } else {
            "gnss-pill ok"
        };
        let rain_txt = if wp.precipitation_mm_h > 0.0 {
            format!(" · 💧 {:.1} mm/h", wp.precipitation_mm_h)
        } else {
            String::new()
        };

        wp_cards_html.push_str(&format!(
            r#"<div id="wp-card-{idx}" class="{card_cls}" onclick="focusWaypoint({idx})">
                <div class="wp-top">
                    <span class="wp-badge">{icon} #{num}</span>
                    <span class="wp-time">🕒 {ts}</span>
                </div>
                <div class="wp-loc">{loc}</div>
                <div class="wp-meta">📍 <code>{lat:.5}°N, {lon:.5}°E</code> · 🚛 {spd} km/h{temp}</div>
                <div class="wp-pills">
                    <span class="{gnss_badge_cls}">🛰️ {sats} Sat. · HDOP {hdop:.1} · ±{acc:.1} m ({fix})</span>
                    <span class="weather-pill">{weather} · 🌡️ {amb:.1}°C{rain} · 💨 {wind} km/h</span>
                </div>
            </div>"#,
            card_cls = card_cls,
            idx = idx,
            icon = icon,
            num = idx + 1,
            ts = wp.timestamp,
            loc = wp.location_name,
            lat = wp.lat,
            lon = wp.lon,
            spd = wp.speed_kmh,
            temp = temp_txt,
            gnss_badge_cls = gnss_badge_cls,
            sats = wp.satellites_used,
            hdop = wp.hdop,
            acc = wp.gps_accuracy_m,
            fix = wp.fix_type,
            weather = wp.weather_condition,
            amb = wp.ambient_temp_c,
            rain = rain_txt,
            wind = wp.wind_speed_kmh
        ));
    }

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="de">
<head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>GPS-Tour {tour_id} ({origin} -> {dest}) - OpenStreetMap 1-Hz & Wetter</title>
    <link rel="stylesheet" href="https://unpkg.com/leaflet@1.9.4/dist/leaflet.css" />
    <script src="https://unpkg.com/leaflet@1.9.4/dist/leaflet.js"></script>
    <style>
        * {{ box-sizing: border-box; margin: 0; padding: 0; }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            background: #0b0f19;
            color: #f8fafc;
            height: 100vh;
            display: grid;
            grid-template-columns: 440px 1fr;
            overflow: hidden;
        }}
        .sidebar {{
            background: #111827;
            border-right: 1px solid #334155;
            display: flex;
            flex-direction: column;
            height: 100vh;
            overflow: hidden;
        }}
        .sb-header {{
            padding: 0.9rem 1.1rem;
            border-bottom: 1px solid #334155;
            background: #0f172a;
        }}
        .sb-title {{
            font-size: 0.98rem;
            font-weight: 700;
            color: #38bdf8;
            margin-bottom: 0.3rem;
        }}
        .sb-sub {{
            font-size: 0.78rem;
            color: #cbd5e1;
            line-height: 1.42;
        }}
        .telemetry-summary {{
            display: grid;
            grid-template-columns: 1fr 1fr;
            gap: 0.35rem;
            margin-top: 0.55rem;
            padding: 0.45rem 0.55rem;
            background: #1e293b;
            border: 1px solid #334155;
            border-radius: 0.45rem;
            font-size: 0.72rem;
        }}
        .gmaps-btn {{
            display: flex;
            align-items: center;
            justify-content: center;
            gap: 0.45rem;
            margin-top: 0.45rem;
            padding: 0.45rem 0.8rem;
            background: linear-gradient(135deg, #2563eb 0%, #1d4ed8 100%);
            color: #fff;
            text-decoration: none;
            font-size: 0.78rem;
            font-weight: 700;
            border-radius: 0.45rem;
            border: 1px solid #60a5fa;
        }}
        .gmaps-btn:hover {{ opacity: 0.92; }}
        .anomaly-btn {{
            padding: 0.46rem 0.65rem;
            background: rgba(239, 68, 68, 0.2);
            border: 1px solid #ef4444;
            color: #fecaca;
            font-size: 0.74rem;
            font-weight: 700;
            border-radius: 0.45rem;
            cursor: pointer;
            text-align: center;
        }}
        .anomaly-btn:hover {{ background: rgba(239, 68, 68, 0.32); }}
        .replay-btn {{
            padding: 0.46rem 0.75rem;
            background: rgba(16, 185, 129, 0.22);
            border: 1px solid #10b981;
            color: #a7f3d0;
            font-size: 0.74rem;
            font-weight: 700;
            border-radius: 0.45rem;
            cursor: pointer;
            white-space: nowrap;
        }}
        .replay-btn:hover {{ background: rgba(16, 185, 129, 0.35); }}
        .wp-list {{
            flex: 1;
            overflow-y: auto;
            padding: 0.75rem;
            display: flex;
            flex-direction: column;
            gap: 0.5rem;
        }}
        .wp-card {{
            background: #1e293b;
            border: 1px solid #334155;
            border-radius: 0.55rem;
            padding: 0.6rem 0.75rem;
            cursor: pointer;
            transition: all 0.15s ease;
        }}
        .wp-card:hover {{
            border-color: #38bdf8;
            background: #26354d;
        }}
        .wp-card-alert {{
            background: rgba(239, 68, 68, 0.12);
            border-color: #ef4444;
        }}
        .wp-top {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 0.22rem;
            font-size: 0.73rem;
        }}
        .wp-badge {{
            background: rgba(56, 189, 248, 0.18);
            color: #38bdf8;
            padding: 0.08rem 0.42rem;
            border-radius: 4px;
            font-weight: 700;
        }}
        .wp-time {{ color: #94a3b8; font-family: monospace; }}
        .wp-loc {{
            font-size: 0.8rem;
            font-weight: 600;
            color: #f8fafc;
            margin-bottom: 0.22rem;
        }}
        .wp-meta {{
            font-size: 0.72rem;
            color: #cbd5e1;
            margin-bottom: 0.3rem;
        }}
        .wp-meta code {{
            color: #fde047;
            background: #0f172a;
            padding: 0.06rem 0.28rem;
            border-radius: 3px;
        }}
        .wp-pills {{
            display: flex;
            flex-direction: column;
            gap: 0.22rem;
            font-size: 0.7rem;
        }}
        .gnss-pill {{
            padding: 0.12rem 0.42rem;
            border-radius: 4px;
            font-weight: 600;
        }}
        .gnss-pill.ok {{
            background: rgba(16, 185, 129, 0.15);
            color: #6ee7b7;
            border: 1px solid rgba(16, 185, 129, 0.35);
        }}
        .gnss-pill.alert {{
            background: rgba(239, 68, 68, 0.22);
            color: #fca5a5;
            border: 1px solid #ef4444;
        }}
        .weather-pill {{
            background: rgba(56, 189, 248, 0.12);
            color: #bae6fd;
            border: 1px solid rgba(56, 189, 248, 0.3);
            padding: 0.12rem 0.42rem;
            border-radius: 4px;
        }}
        .map-wrap {{
            position: relative;
            width: 100%;
            height: 100vh;
        }}
        #map {{
            width: 100%;
            height: 100vh;
            background: #0f172a;
        }}
        .hud-box {{
            position: absolute;
            top: 14px;
            right: 14px;
            z-index: 1000;
            background: rgba(15, 23, 42, 0.92);
            border: 1px solid #38bdf8;
            border-radius: 0.65rem;
            padding: 0.7rem 0.9rem;
            min-width: 310px;
            max-width: 390px;
            box-shadow: 0 10px 25px rgba(0,0,0,0.55);
            font-size: 0.76rem;
            line-height: 1.48;
            backdrop-filter: blur(8px);
        }}
    </style>
</head>
<body>
    <div class="sidebar">
        <div class="sb-header">
            <div class="sb-title">🛰️ {tour_id} · {truck_id} ({plate})</div>
            <div class="sb-sub">
                <div><strong>Route:</strong> {origin} ({owh}) ➔ {dest} ({dwh})</div>
                <div><strong>Fahrer:</strong> {driver} · <strong>Distanz:</strong> {dist} km</div>
                <div><strong>Kunde:</strong> {cust} · <strong>Fracht:</strong> {cargo} ({tons:.1} t)</div>
                <div style="margin-top:0.2rem; color:{line_color}; font-weight:700;">{status}</div>
            </div>
            <div class="telemetry-summary">
                <div>📡 <strong>Punkte (1 Hz):</strong> {wp_count} Sek.-Punkte</div>
                <div>🛰️ <strong>Min. Satelliten:</strong> {min_sats} Sat. ({low_sat_count}s &lt;4 Sat.)</div>
                <div>🎯 <strong>Max. Ungenauigkeit:</strong> ±{max_acc_m:.1} m</div>
                <div>⛈️ <strong>Max. Regen / Wind:</strong> {max_rain:.1} mm/h · {max_wind} km/h</div>
            </div>
            {anomaly_zoom_btn}
            <a class="gmaps-btn" href="{gmaps}" target="_blank" rel="noopener noreferrer">
                🌍 Route in Google Maps öffnen ↗️
            </a>
        </div>
        <div class="wp-list">
            {wp_cards_html}
        </div>
    </div>
    <div class="map-wrap">
        <div id="map"></div>
        <div id="hudBox" class="hud-box">
            <div style="font-weight:700; color:#38bdf8; margin-bottom:0.2rem;">📡 Live 1-Hz Telemetrie-Monitor</div>
            <div id="hudContent" style="color:#e2e8f0;">Klicke links auf eine Sekunde oder starte <b>▶️ 1-Hz Replay</b>.</div>
        </div>
    </div>
    <script>
        const waypoints = {waypoints_json};
        const map = L.map('map');
        L.tileLayer('https://{{s}}.tile.openstreetmap.org/{{z}}/{{x}}/{{y}}.png', {{
            maxZoom: 19,
            attribution: '&copy; OpenStreetMap contributors'
        }}).addTo(map);

        const latlngs = waypoints.map(w => [w.lat, w.lon]);
        const markers = [];
        const isDense1Hz = waypoints.length > 50;

        if (isDense1Hz) {{
            const trueRennsteig = [
                [50.6050, 10.7150],
                [50.6780, 10.7620],
                [50.7520, 10.8120]
            ];
            L.polyline(trueRennsteig, {{
                color: '#10b981',
                weight: 3.5,
                dashArray: '6, 6',
                opacity: 0.9
            }}).addTo(map).bindPopup('<b>Reale A71 Rennsteigtunnel-Röhre (7,9 km Soll-Spur)</b><br/>Grün gestrichelt = echte Tunnelröhre.<br/>Rot = abgedriftete 1-Hz IMU-Spur (0 Satelliten, bis ±118.5 m Drift).');

            const trueElbtunnel = [
                [53.5360, 9.9212],
                [53.5445, 9.9420],
                [53.5413, 9.9841]
            ];
            L.polyline(trueElbtunnel, {{
                color: '#10b981',
                weight: 3.5,
                dashArray: '6, 6',
                opacity: 0.9
            }}).addTo(map).bindPopup('<b>Reale A7 Hamburger Elbtunnel-Röhre (Soll-Spur)</b><br/>Grün gestrichelt = echte A7-Röhre unter der Elbe.<br/>Rot = abgedriftete 1-Hz IMU-Spur (0 Satelliten, bis ±124.7 m Drift).');
        }}

        for (let i = 0; i + 1 < waypoints.length; i++) {{
            const w1 = waypoints[i];
            const w2 = waypoints[i + 1];
            let segColor = '#0ea5e9';
            if (w2.satellites_used === 0 || w1.satellites_used === 0) {{
                segColor = '#ef4444';
            }} else if (w2.satellites_used < 4 || w2.gps_accuracy_m >= 15.0) {{
                segColor = '#f59e0b';
            }}
            L.polyline([[w1.lat, w1.lon], [w2.lat, w2.lon]], {{
                color: segColor,
                weight: isDense1Hz ? 3.5 : 5,
                opacity: 0.85
            }}).addTo(map);
        }}

        waypoints.forEach((w, i) => {{
            const isStart = i === 0;
            const isEnd = i === waypoints.length - 1;
            const isZeroSat = w.satellites_used === 0;
            const isBadGnss = (w.satellites_used < 4) || (w.gps_accuracy_m >= 15.0);
            const color = isStart ? '#10b981' : (isEnd ? '#6366f1' : (isZeroSat ? '#ef4444' : (isBadGnss ? '#f59e0b' : '#38bdf8')));

            const drawCircle = isDense1Hz
                ? (isBadGnss && (i % 6 === 0))
                : (w.gps_accuracy_m >= 5.0 || w.satellites_used < 4);
            if (drawCircle) {{
                L.circle([w.lat, w.lon], {{
                    radius: w.gps_accuracy_m,
                    color: '#ef4444',
                    weight: 1.3,
                    dashArray: '4, 4',
                    fillColor: '#ef4444',
                    fillOpacity: 0.18
                }}).addTo(map);
            }}

            const markerRadius = (isStart || isEnd) ? 9 : (isDense1Hz ? (isBadGnss ? 5.0 : 3.0) : (isBadGnss ? 8 : 6));
            const marker = L.circleMarker([w.lat, w.lon], {{
                radius: markerRadius,
                fillColor: color,
                color: (isStart || isEnd || !isDense1Hz) ? '#ffffff' : color,
                weight: (isStart || isEnd || !isDense1Hz) ? 2 : 0,
                opacity: 1,
                fillOpacity: 0.95
            }}).addTo(map);

            const tempStr = (w.temp_c !== null && w.temp_c !== undefined) ? `<br/>❄️ Kühlraum-Temp.: <b>${{w.temp_c.toFixed(1)}} °C</b>` : '';
            const rainStr = (w.precipitation_mm_h > 0) ? ` · Niederschlag: <b>${{w.precipitation_mm_h.toFixed(1)}} mm/h</b>` : '';
            marker.bindPopup(`
                <div style="font-family:sans-serif; font-size:12.5px; line-height:1.48; min-width:240px;">
                    <strong>#${{i + 1}}: ${{w.location_name}}</strong><br/>
                    🕒 Zeit (1-Hz): <b>${{w.timestamp}}</b><br/>
                    📍 GPS: <code>${{w.lat.toFixed(5)}}°N, ${{w.lon.toFixed(5)}}°E</code><br/>
                    🛰️ GNSS: <b>${{w.satellites_used}} Satelliten</b> · HDOP: <b>${{w.hdop.toFixed(1)}}</b><br/>
                    🎯 Genauigkeit: <b>±${{w.gps_accuracy_m.toFixed(1)}} m</b> (<code>${{w.fix_type}}</code>)<br/>
                    ☁️ Wetter: <b>${{w.weather_condition}}</b><br/>
                    🌡️ Außentemp.: <b>${{w.ambient_temp_c.toFixed(1)}} °C</b>${{rainStr}} · Wind: <b>${{w.wind_speed_kmh}} km/h</b><br/>
                    🚛 Geschwindigkeit: <b>${{w.speed_kmh}} km/h</b>${{tempStr}}
                </div>
            `);
            marker.on('click', () => updateHud(i));
            markers.push(marker);
        }});

        let activeCircle = null;
        let activeTruckMarker = null;

        function updateHud(idx) {{
            const w = waypoints[idx];
            if (!w) return;
            const satColor = w.satellites_used < 4 ? '#fca5a5' : '#6ee7b7';
            const hud = document.getElementById('hudContent');
            if (hud) {{
                hud.innerHTML = `
                    <div><strong>Punkt #${{idx + 1}} / ${{waypoints.length}}</strong> · 🕒 <code>${{w.timestamp}}</code></div>
                    <div style="color:#fde047; font-weight:600;">${{w.location_name}}</div>
                    <div>📍 <code>${{w.lat.toFixed(5)}}°N, ${{w.lon.toFixed(5)}}°E</code> · 🚛 <b>${{w.speed_kmh}} km/h</b></div>
                    <div style="color:${{satColor}}; font-weight:700;">🛰️ ${{w.satellites_used}} Satelliten · HDOP ${{w.hdop.toFixed(1)}} · ±${{w.gps_accuracy_m.toFixed(1)}} m (${{w.fix_type}})</div>
                    <div>${{w.weather_condition}} · 🌡️ ${{w.ambient_temp_c.toFixed(1)}}°C · 💧 ${{w.precipitation_mm_h.toFixed(1)}} mm/h · 💨 ${{w.wind_speed_kmh}} km/h</div>
                `;
            }}
            if (activeCircle) map.removeLayer(activeCircle);
            activeCircle = L.circle([w.lat, w.lon], {{
                radius: w.gps_accuracy_m,
                color: '#fde047',
                weight: 2.5,
                fillColor: '#ef4444',
                fillOpacity: 0.28
            }}).addTo(map);

            if (activeTruckMarker) map.removeLayer(activeTruckMarker);
            activeTruckMarker = L.circleMarker([w.lat, w.lon], {{
                radius: 8,
                fillColor: '#fde047',
                color: '#0f172a',
                weight: 2.5,
                opacity: 1,
                fillOpacity: 1
            }}).addTo(map);
        }}

        function fitFullRoute() {{
            if (latlngs.length > 0) {{
                const fullBounds = L.latLngBounds(latlngs);
                map.fitBounds(fullBounds, {{ padding: [40, 40] }});
                updateHud(isDense1Hz ? 220 : 0);
            }}
        }}

        fitFullRoute();

        function focusWaypoint(idx) {{
            if (markers[idx]) {{
                const w = waypoints[idx];
                const zoomLvl = (w.satellites_used < 4 || w.gps_accuracy_m >= 15.0) ? 12 : 10;
                map.setView([w.lat, w.lon], zoomLvl, {{ animate: true }});
                markers[idx].openPopup();
                updateHud(idx);
            }}
        }}

        function focusAnomalyZone(idx) {{
            if (markers[idx]) {{
                const w = waypoints[idx];
                map.setView([w.lat, w.lon], 11, {{ animate: true }});
                markers[idx].openPopup();
                updateHud(idx);
                const card = document.getElementById('wp-card-' + idx);
                if (card) card.scrollIntoView({{ behavior: 'smooth', block: 'center' }});
            }}
        }}

        let replayTimer = null;
        let replayIdx = 0;
        function toggleReplay() {{
            const btn = document.getElementById('replayBtn');
            if (replayTimer) {{
                clearInterval(replayTimer);
                replayTimer = null;
                if (btn) btn.innerHTML = '▶️ 1-Hz Replay';
                return;
            }}
            if (btn) btn.innerHTML = '⏸️ Pause';
            if (replayIdx >= waypoints.length) replayIdx = 0;
            replayTimer = setInterval(() => {{
                if (replayIdx >= waypoints.length) {{
                    clearInterval(replayTimer);
                    replayTimer = null;
                    if (btn) btn.innerHTML = '▶️ 1-Hz Replay';
                    return;
                }}
                updateHud(replayIdx);
                const card = document.getElementById('wp-card-' + replayIdx);
                if (card && replayIdx % 8 === 0) {{
                    card.scrollIntoView({{ behavior: 'smooth', block: 'center' }});
                }}
                replayIdx += 1;
            }}, 90);
        }}
    </script>
</body>
</html>"#,
        tour_id = tour.tour_id,
        truck_id = tour.truck_id,
        plate = tour.license_plate,
        origin = tour.origin_city,
        owh = tour.origin_warehouse_id,
        dest = tour.destination_city,
        dwh = tour.destination_warehouse_id,
        driver = tour.driver_name,
        dist = tour.distance_km,
        cust = tour.customer_name,
        cargo = tour.cargo_description,
        tons = tour.cargo_weight_tons,
        status = tour.delivery_status,
        wp_count = wp_count,
        min_sats = min_sats,
        low_sat_count = low_sat_count,
        max_acc_m = max_acc_m,
        max_rain = max_rain,
        max_wind = max_wind,
        anomaly_zoom_btn = anomaly_zoom_btn,
        gmaps = tour.google_maps_url,
        line_color = line_color,
        wp_cards_html = wp_cards_html,
        waypoints_json = waypoints_json
    );

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(html))
        .unwrap()
}
