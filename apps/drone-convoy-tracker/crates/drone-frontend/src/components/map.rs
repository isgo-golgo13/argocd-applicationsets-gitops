//! # Map Component
//!
//! Afghanistan tactical map with drone markers using Leaflet.js.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;

use crate::state::use_app_state;

/// Leaflet map wrapper
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = L)]
    type Map;

    #[wasm_bindgen(js_namespace = L, js_name = map)]
    fn create_map(id: &str) -> Map;

    #[wasm_bindgen(method, js_name = setView)]
    fn set_view(this: &Map, lat_lng: &JsValue, zoom: u32) -> Map;

    #[wasm_bindgen(js_namespace = L, js_name = tileLayer)]
    fn tile_layer(url: &str, options: &JsValue) -> TileLayer;

    #[wasm_bindgen]
    type TileLayer;

    #[wasm_bindgen(method, js_name = addTo)]
    fn add_to(this: &TileLayer, map: &Map);

    #[wasm_bindgen(js_namespace = L)]
    type Marker;

    #[wasm_bindgen(js_namespace = L, js_name = marker)]
    fn create_marker(lat_lng: &JsValue, options: &JsValue) -> Marker;

    #[wasm_bindgen(method, js_name = addTo)]
    fn marker_add_to(this: &Marker, map: &Map);

    #[wasm_bindgen(method, js_name = bindPopup)]
    fn bind_popup(this: &Marker, content: &str) -> Marker;

    #[wasm_bindgen(method, js_name = setLatLng)]
    fn set_lat_lng(this: &Marker, lat_lng: &JsValue);

    #[wasm_bindgen(js_namespace = L)]
    type Polyline;

    #[wasm_bindgen(js_namespace = L, js_name = polyline)]
    fn create_polyline(lat_lngs: &JsValue, options: &JsValue) -> Polyline;

    #[wasm_bindgen(method, js_name = addTo)]
    fn polyline_add_to(this: &Polyline, map: &Map);
    
    #[wasm_bindgen(js_namespace = L)]
    type Circle;
    
    #[wasm_bindgen(js_namespace = L, js_name = circle)]
    fn create_circle(lat_lng: &JsValue, options: &JsValue) -> Circle;
    
    #[wasm_bindgen(method, js_name = addTo)]
    fn circle_add_to(this: &Circle, map: &Map);
}

/// Check if Leaflet is loaded
fn leaflet_available() -> bool {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return false,
    };
    
    match js_sys::Reflect::get(&window, &JsValue::from_str("L")) {
        Ok(val) => !val.is_undefined() && !val.is_null(),
        Err(_) => false,
    }
}

/// Afghanistan map panel
#[component]
pub fn MapPanel() -> impl IntoView {
    let state = use_app_state();
    let map_id = "tactical-map";

    // Center on Kandahar Province, Afghanistan
    let center_lat = 31.6289;
    let center_lng = 65.7372;
    let aor_radius_m = 150_000.0; // 150km AOR radius

    // Initialize map after a small delay to ensure DOM is ready
    Effect::new(move |_| {
        // Use setTimeout to ensure DOM element exists
        let closure = Closure::once(Box::new(move || {
            if !leaflet_available() {
                log::error!("Leaflet not loaded!");
                return;
            }

            // Check if map element exists
            let document = web_sys::window().unwrap().document().unwrap();
            if document.get_element_by_id(map_id).is_none() {
                log::error!("Map element not found: {}", map_id);
                return;
            }

            // Create map
            let map = create_map(map_id);
            let center = js_sys::Array::new();
            center.push(&JsValue::from_f64(center_lat));
            center.push(&JsValue::from_f64(center_lng));
            map.set_view(&center.into(), 8);

            // Add dark tile layer (CartoDB Dark Matter)
            let tile_options = js_sys::Object::new();
            js_sys::Reflect::set(&tile_options, &"maxZoom".into(), &19.into()).unwrap();
            js_sys::Reflect::set(&tile_options, &"attribution".into(), &"© OpenStreetMap © CartoDB".into()).unwrap();
            
            let tiles = tile_layer(
                "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}",
                &tile_options.into(),
            );
            tiles.add_to(&map);

            // Add labels layer on top
            let label_options = js_sys::Object::new();
            js_sys::Reflect::set(&label_options, &"maxZoom".into(), &19.into()).unwrap();
            js_sys::Reflect::set(&label_options, &"pane".into(), &"overlayPane".into()).unwrap();
            let labels = tile_layer(
                "https://{s}.basemaps.cartocdn.com/dark_only_labels/{z}/{x}/{y}{r}.png",
                &label_options.into(),
            );
            labels.add_to(&map);

            // Add AOR circle (150km radius around Kandahar)
            let aor_center = js_sys::Array::new();
            aor_center.push(&JsValue::from_f64(center_lat));
            aor_center.push(&JsValue::from_f64(center_lng));
            
            let aor_options = js_sys::Object::new();
            js_sys::Reflect::set(&aor_options, &"radius".into(), &JsValue::from_f64(aor_radius_m)).unwrap();
            js_sys::Reflect::set(&aor_options, &"color".into(), &"#00ff41".into()).unwrap();
            js_sys::Reflect::set(&aor_options, &"fillColor".into(), &"#00ff41".into()).unwrap();
            js_sys::Reflect::set(&aor_options, &"fillOpacity".into(), &JsValue::from_f64(0.05)).unwrap();
            js_sys::Reflect::set(&aor_options, &"weight".into(), &JsValue::from_f64(2.0)).unwrap();
            js_sys::Reflect::set(&aor_options, &"dashArray".into(), &"5, 10".into()).unwrap();
            
            let aor_circle = create_circle(&aor_center.into(), &aor_options.into());
            aor_circle.circle_add_to(&map);

            // Add drone markers
            let drones = state.drones.get();
            for drone in drones.values() {
                let pos = &drone.position;
                let marker_pos = js_sys::Array::new();
                marker_pos.push(&JsValue::from_f64(pos.latitude));
                marker_pos.push(&JsValue::from_f64(pos.longitude));

                let marker_options = js_sys::Object::new();
                let marker = create_marker(&marker_pos.into(), &marker_options.into());
                
                let popup_content = format!(
                    "<div style='font-family: monospace; color: #00ff41; background: #0a0f0d; padding: 8px; border: 1px solid #00ff41;'>\
                    <b style='color: #00ff41;'>{}</b><br/>\
                    <span style='color: #557755;'>ALT:</span> {:.0}m<br/>\
                    <span style='color: #557755;'>HDG:</span> {:.0}°<br/>\
                    <span style='color: #557755;'>SPD:</span> {:.0} m/s<br/>\
                    <span style='color: #557755;'>FUEL:</span> {:.1}%\
                    </div>",
                    drone.callsign, pos.altitude_m, pos.heading_deg, pos.speed_mps, drone.fuel_pct
                );
                marker.bind_popup(&popup_content);
                marker.marker_add_to(&map);
            }

            log::info!("Map initialized with {} drone markers", drones.len());
        }) as Box<dyn FnOnce()>);

        let window = web_sys::window().unwrap();
        window
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                closure.as_ref().unchecked_ref(),
                100,
            )
            .unwrap();
        closure.forget(); // Prevent closure from being dropped
    });

    let selected_drone = move || state.selected_drone.get();
    let drone_position = move || {
        selected_drone().and_then(|id| {
            state.drones.get().get(&id).map(|d| d.position.clone())
        })
    };

    view! {
        <div class="map-container">
            <div id=map_id class="leaflet-map"></div>

            <div class="map-overlay">
                <div class="map-control">
                    <span class="status-dot nominal"></span>
                    "KANDAHAR AOR"
                </div>

                {move || drone_position().map(|pos| view! {
                    <div class="map-control">
                        <span class="text-accent">"SEL:"</span>
                        {format!("{:.4}°N {:.4}°E", pos.latitude, pos.longitude)}
                    </div>
                })}
            </div>

            <div style="position: absolute; bottom: 16px; right: 16px; z-index: 1000;">
                <div class="map-control" style="font-size: 0.7rem;">
                    <span class="text-muted">"ALT:"</span>
                    {move || drone_position().map(|p| format!("{:.0}m", p.altitude_m)).unwrap_or_else(|| "---".to_string())}
                    " "
                    <span class="text-muted">"HDG:"</span>
                    {move || drone_position().map(|p| format!("{:.0}°", p.heading_deg)).unwrap_or_else(|| "---".to_string())}
                </div>
            </div>
        </div>
    }
}

/// Map fallback when Leaflet not loaded
#[component]
pub fn MapFallback() -> impl IntoView {
    view! {
        <div class="map-container" style="display: flex; align-items: center; justify-content: center;">
            <div style="text-align: center;">
                <div class="spinner" style="margin: 0 auto 16px;"></div>
                <div class="text-muted">"Loading tactical map..."</div>
            </div>
        </div>
    }
}
