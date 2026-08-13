//! # Map Component
//!
//! Afghanistan tactical map with drone markers using Leaflet.js.

use leptos::prelude::*;
use wasm_bindgen::prelude::*;

use crate::state::use_app_state;

/// Airframe marker, compiled in from assets/images so there is no runtime fetch
/// and no asset path to get wrong in the WASM bundle.
const DRONE_SVG: &str = include_str!("../../../../assets/images/drone.svg");

/// Marker accent per status. Light red is the default so drones read as hostile
/// air against the green HUD; amber and grey call out the exceptions.
fn status_accent(status: &crate::state::DroneStatus) -> &'static str {
    use crate::state::DroneStatus;
    match status {
        DroneStatus::Rtb | DroneStatus::Egress => "#ffae2b",
        DroneStatus::Landed | DroneStatus::Maintenance | DroneStatus::Preflight => "#6b7280",
        DroneStatus::Airborne | DroneStatus::Loiter | DroneStatus::Ingress => "#ff5f5f",
    }
}

/// Strip the XML prolog: valid in a standalone file, invalid inside innerHTML.
fn inline_svg(svg: &str) -> &str {
    svg.find("<svg").map_or(svg, |i| &svg[i..])
}

/// Sortie route across the Kandahar AOR, ingress in the north-west, target run
/// through the centre, egress south-east.
///
/// Client-side for now: the `waypoints` resolver is still a stub, and its
/// repository method takes `_drone_id` and returns an empty vec. When that is
/// wired up this constant is replaced by the query result and nothing else in
/// this file changes.
const ROUTE: [(f64, f64); 14] = [
    (31.9800, 64.9000),
    (31.9100, 65.0600),
    (31.8300, 65.2100),
    (31.7600, 65.3500),
    (31.7000, 65.4900),
    (31.6600, 65.6200),
    (31.6289, 65.7372),
    (31.5900, 65.8500),
    (31.5400, 65.9600),
    (31.4800, 66.0600),
    (31.4100, 66.1500),
    (31.3400, 66.2300),
    (31.2600, 66.3100),
    (31.1800, 66.3900),
];

/// Spacing between drones, in route legs. Enough to read as line astern
/// without the airframes overlapping at map zoom 8.
const CONVOY_SPACING_LEGS: f64 = 0.55;

/// Animation tick. 120ms is smooth without flooding Leaflet with layer updates.
const FLIGHT_TICK_MS: i32 = 120;

/// Legs advanced per tick. The full route takes roughly three minutes.
const LEGS_PER_TICK: f64 = 0.006;

/// Position and heading at `progress` legs along ROUTE, wrapping at the end.
///
/// Returns the interpolated point plus a compass heading derived from the leg
/// direction, so the airframe always points where it is going.
fn route_point(progress: f64) -> (f64, f64, f64) {
    let legs = (ROUTE.len() - 1) as f64;
    let wrapped = progress.rem_euclid(legs);
    let idx = wrapped.floor() as usize;
    let frac = wrapped - wrapped.floor();

    let (lat_a, lng_a) = ROUTE[idx];
    let (lat_b, lng_b) = ROUTE[(idx + 1).min(ROUTE.len() - 1)];

    let lat = lat_a + (lat_b - lat_a) * frac;
    let lng = lng_a + (lng_b - lng_a) * frac;

    // Longitude degrees shrink with latitude; without the cos correction the
    // heading is visibly wrong at this latitude.
    let d_lng = (lng_b - lng_a) * lat.to_radians().cos();
    let d_lat = lat_b - lat_a;
    let heading = d_lng.atan2(d_lat).to_degrees().rem_euclid(360.0);

    (lat, lng, heading)
}

fn lat_lng(lat: f64, lng: f64) -> JsValue {
    let arr = js_sys::Array::new();
    arr.push(&JsValue::from_f64(lat));
    arr.push(&JsValue::from_f64(lng));
    arr.into()
}

/// A divIcon carrying arbitrary HTML at a given square size, anchored at its
/// centre. Use `html_icon_at` for markers that hang from a point.
fn html_icon(html: &str, class: &str, size: f64) -> DivIcon {
    html_icon_at(html, class, size, size / 2.0, size / 2.0)
}

/// As `html_icon`, with an explicit anchor within the icon box.
fn html_icon_at(html: &str, class: &str, size: f64, anchor_x: f64, anchor_y: f64) -> DivIcon {
    let opts = js_sys::Object::new();
    js_sys::Reflect::set(&opts, &"html".into(), &html.into()).ok();
    js_sys::Reflect::set(&opts, &"className".into(), &class.into()).ok();
    let dims = js_sys::Array::new();
    dims.push(&JsValue::from_f64(size));
    dims.push(&JsValue::from_f64(size));
    js_sys::Reflect::set(&opts, &"iconSize".into(), &dims.into()).ok();
    let anchor = js_sys::Array::new();
    anchor.push(&JsValue::from_f64(anchor_x));
    anchor.push(&JsValue::from_f64(anchor_y));
    js_sys::Reflect::set(&opts, &"iconAnchor".into(), &anchor.into()).ok();
    div_icon(&opts.into())
}

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

    #[wasm_bindgen(method, js_name = getElement)]
    fn get_element(this: &Marker) -> JsValue;

    #[wasm_bindgen(js_namespace = L)]
    type DivIcon;

    #[wasm_bindgen(js_namespace = L, js_name = divIcon)]
    fn div_icon(options: &JsValue) -> DivIcon;

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

            // ---------------------------------------------------------------
            // Sortie route: red waypoint pins plus the track between them.
            // ---------------------------------------------------------------
            let track = js_sys::Array::new();
            for (lat, lng) in ROUTE {
                track.push(&lat_lng(lat, lng));
            }
            let track_opts = js_sys::Object::new();
            js_sys::Reflect::set(&track_opts, &"color".into(), &"#ff5f5f".into()).ok();
            js_sys::Reflect::set(&track_opts, &"weight".into(), &JsValue::from_f64(1.5)).ok();
            js_sys::Reflect::set(&track_opts, &"opacity".into(), &JsValue::from_f64(0.45)).ok();
            js_sys::Reflect::set(&track_opts, &"dashArray".into(), &"6, 8".into()).ok();
            create_polyline(&track.into(), &track_opts.into()).polyline_add_to(&map);

            for (i, (lat, lng)) in ROUTE.iter().enumerate() {
                let pin_opts = js_sys::Object::new();
                js_sys::Reflect::set(
                    &pin_opts,
                    &"icon".into(),
                    // Anchored at the bottom tip so the point sits on the
                    // waypoint, not the centre of the teardrop.
                    &JsValue::from(html_icon_at(
                        "<div class=\"wp-pin\"></div>",
                        "waypoint-div-icon",
                        16.0,
                        8.0,
                        16.0,
                    )),
                )
                .ok();
                let pin = create_marker(&lat_lng(*lat, *lng), &pin_opts.into());
                pin.bind_popup(&format!("WP {}", i + 1));
                pin.marker_add_to(&map);
            }

            // ---------------------------------------------------------------
            // Convoy: one marker per drone, flying the route line astern.
            //
            // Markers are created INCREMENTALLY. At mount the drones map is
            // empty — state fills from the 2s poll — so a one-shot snapshot
            // here would put zero airframes on the map forever. A sync
            // interval watches state and adds a marker for every callsign it
            // hasn't seen; the flight loop animates whatever exists.
            // ---------------------------------------------------------------
            use std::cell::RefCell;
            use std::rc::Rc;

            let markers: Rc<RefCell<Vec<(String, Marker)>>> = Rc::new(RefCell::new(Vec::new()));

            let sync = {
                let markers = Rc::clone(&markers);
                // `map` is captured by move: wasm_bindgen extern types have no
                // own Clone (a .clone() derefs to JsValue and loses the type),
                // and nothing after this closure uses the map handle.
                move || {
                    let drones = state.drones.get_untracked();
                    let mut ordered: Vec<_> = drones.values().cloned().collect();
                    // Stable order, so a drone keeps its slot in the formation
                    // instead of shuffling with HashMap iteration order.
                    ordered.sort_by(|a, b| a.callsign.cmp(&b.callsign));

                    let mut known = markers.borrow_mut();
                    for drone in &ordered {
                        let accent = status_accent(&drone.status);
                        let popup = format!(
                            "<div style='font-family: monospace; color: #00ff41; background: #0a0f0d; \
                             padding: 8px; border: 1px solid #00ff41;'>\
                             <b>{}</b><br/><span style='color:#557755;'>FUEL:</span> {:.0}%\
                             <br/><span style='color:#557755;'>ACC:</span> {:.1}%</div>",
                            drone.callsign, drone.fuel_pct, drone.accuracy_pct
                        );

                        if let Some((_, marker)) =
                            known.iter().find(|(cs, _)| cs == &drone.callsign)
                        {
                            // Existing airframe: refresh the popup so FUEL/ACC
                            // track the live values instead of the join-time
                            // snapshot.
                            marker.bind_popup(&popup);
                            continue;
                        }

                        let html = format!(
                            "<div class=\"drone-air-marker\" style=\"--drone-accent:{accent};\
                             width:34px;height:34px;filter:drop-shadow(0 0 5px {accent});\">{svg}</div>",
                            accent = accent,
                            svg = inline_svg(DRONE_SVG),
                        );

                        let opts = js_sys::Object::new();
                        js_sys::Reflect::set(
                            &opts,
                            &"icon".into(),
                            &JsValue::from(html_icon(&html, "drone-div-icon", 34.0)),
                        )
                        .ok();

                        let marker = create_marker(&lat_lng(ROUTE[0].0, ROUTE[0].1), &opts.into());
                        marker.bind_popup(&popup);
                        marker.marker_add_to(&map);

                        // Insert in callsign order so the formation slot is
                        // deterministic no matter the join order.
                        let at = known
                            .iter()
                            .position(|(cs, _)| cs.as_str() > drone.callsign.as_str())
                            .unwrap_or(known.len());
                        known.insert(at, (drone.callsign.clone(), marker));
                        log::info!("map: airframe joined — {}", drone.callsign);
                    }
                }
            };
            // First sync immediately (covers a fast poll), then every second.
            sync();
            let sync_closure = Closure::wrap(Box::new(sync) as Box<dyn Fn()>);
            if let Some(window) = web_sys::window() {
                let _ = window.set_interval_with_callback_and_timeout_and_arguments_0(
                    sync_closure.as_ref().unchecked_ref(),
                    1_000,
                );
            }
            sync_closure.forget();

            // ---------------------------------------------------------------
            // Flight loop. Each drone sits CONVOY_SPACING_LEGS behind the one
            // ahead, so the formation reads as line astern along the track.
            // ---------------------------------------------------------------
            let progress = std::rc::Rc::new(std::cell::Cell::new(0.0_f64));
            let tick = {
                let progress = progress.clone();
                let markers = Rc::clone(&markers);
                move || {
                    let t = progress.get() + LEGS_PER_TICK;
                    progress.set(t);

                    for (i, (_callsign, marker)) in markers.borrow().iter().enumerate() {
                        let offset = t - (i as f64) * CONVOY_SPACING_LEGS;
                        if offset < 0.0 {
                            continue; // still holding at the IP
                        }
                        let (lat, lng, heading) = route_point(offset);
                        marker.set_lat_lng(&lat_lng(lat, lng));

                        // Leaflet owns the transform on the icon container, so
                        // rotate the inner element instead of fighting it.
                        let el = marker.get_element();
                        if let Some(el) = el.dyn_ref::<web_sys::Element>() {
                            if let Some(inner) = el.first_element_child() {
                                if let Some(inner) = inner.dyn_ref::<web_sys::HtmlElement>() {
                                    let _ = inner
                                        .style()
                                        .set_property("transform", &format!("rotate({heading:.0}deg)"));
                                }
                            }
                        }
                    }
                }
            };

            let flight = Closure::wrap(Box::new(tick) as Box<dyn Fn()>);
            if let Some(window) = web_sys::window() {
                let _ = window.set_interval_with_callback_and_timeout_and_arguments_0(
                    flight.as_ref().unchecked_ref(),
                    FLIGHT_TICK_MS,
                );
            }
            // Runs for the life of the page; dropping it would kill the callback.
            flight.forget();

            log::info!("map ready: airframes join as they register on a {}-waypoint route", ROUTE.len());
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
