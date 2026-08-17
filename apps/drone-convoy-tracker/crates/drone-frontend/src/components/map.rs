//! # Map Component
//!
//! Tactical map with drone markers using Leaflet.js. Which theater it shows is
//! driven by `state.selected_theater` (the header's mission selector); every
//! theater brings its own centre, AOR ring and sortie route, and switching
//! re-centres, swaps the red pins and track, and restarts the convoy at the
//! new route's IP. Impact bursts render in a dedicated pane BELOW the
//! airframes so an explosion never hides a drone.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use leptos::prelude::*;
use wasm_bindgen::prelude::*;

use crate::components::regions::{Theater, TheaterId};
use crate::state::use_app_state;

/// Airframe marker, compiled in from assets/images so there is no runtime fetch
/// and no asset path to get wrong in the WASM bundle.
const DRONE_SVG: &str = include_str!("../../../../assets/images/drone.svg");

/// Impact burst artwork, same mechanism. Themed per outcome via its
/// `--blast-*` custom properties.
const EXPLOSION_SVG: &str = include_str!("../../../../assets/images/explosion.svg");

/// Leaflet pane for bursts. markerPane is 600; anything lower renders under
/// the airframes. That ordering IS the layering rule.
const IMPACT_PANE: &str = "impact-pane";
const IMPACT_PANE_Z: &str = "550";
/// Burst lifetime must match the CSS keyframe (1200ms) plus a little slack.
const IMPACT_TTL_MS: i32 = 1400;
/// Burst icon size on the map, px.
const IMPACT_SIZE: f64 = 44.0;

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

/// Animation tick. 120ms is smooth without flooding Leaflet with layer updates.
const FLIGHT_TICK_MS: i32 = 120;

/// Poll cadence the flight loop interpolates across; must match lib.rs.
const POLL_INTERVAL_MS: i32 = 2_000;

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

    #[wasm_bindgen(method, js_name = createPane)]
    fn create_pane(this: &Map, name: &str) -> web_sys::HtmlElement;

    #[wasm_bindgen(method, js_name = removeLayer)]
    fn remove_layer(this: &Map, layer: &JsValue) -> Map;

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

    #[wasm_bindgen(method, js_name = on)]
    fn marker_on(this: &Marker, event: &str, handler: &js_sys::Function) -> Marker;

    #[wasm_bindgen(method, js_name = setLatLng)]
    fn set_lat_lng(this: &Marker, lat_lng: &JsValue);

    #[wasm_bindgen(method, js_name = getElement)]
    fn get_element(this: &Marker) -> JsValue;

    #[wasm_bindgen(method, js_name = remove)]
    fn marker_remove(this: &Marker);

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

    #[wasm_bindgen(method, js_name = remove)]
    fn polyline_remove(this: &Polyline);
    
    #[wasm_bindgen(js_namespace = L)]
    type Circle;
    
    #[wasm_bindgen(js_namespace = L, js_name = circle)]
    fn create_circle(lat_lng: &JsValue, options: &JsValue) -> Circle;
    
    #[wasm_bindgen(method, js_name = addTo)]
    fn circle_add_to(this: &Circle, map: &Map);

    #[wasm_bindgen(method, js_name = remove)]
    fn circle_remove(this: &Circle);
}

/// Everything on the map that belongs to ONE theater: the AOR ring, the
/// track and the waypoint pins. Swapping theaters drops the whole set.
struct TheaterLayers {
    aor: Circle,
    track: Polyline,
    pins: Vec<Marker>,
}

impl TheaterLayers {
    fn clear(self) {
        self.aor.circle_remove();
        self.track.polyline_remove();
        for p in self.pins { p.marker_remove(); }
    }
}

/// Draw a theater's AOR ring, track and pins. Pins hang from their tip.
fn draw_theater(map: &Map, t: &Theater) -> TheaterLayers {
    let aor_options = js_sys::Object::new();
    js_sys::Reflect::set(&aor_options, &"radius".into(), &JsValue::from_f64(t.aor_radius_m)).ok();
    js_sys::Reflect::set(&aor_options, &"color".into(), &"#00ff41".into()).ok();
    js_sys::Reflect::set(&aor_options, &"fillColor".into(), &"#00ff41".into()).ok();
    js_sys::Reflect::set(&aor_options, &"fillOpacity".into(), &JsValue::from_f64(0.05)).ok();
    js_sys::Reflect::set(&aor_options, &"weight".into(), &JsValue::from_f64(2.0)).ok();
    js_sys::Reflect::set(&aor_options, &"dashArray".into(), &"5, 10".into()).ok();
    let aor = create_circle(&lat_lng(t.center.0, t.center.1), &aor_options.into());
    aor.circle_add_to(map);

    let track_pts = js_sys::Array::new();
    for (lat, lng) in t.route { track_pts.push(&lat_lng(*lat, *lng)); }
    let track_opts = js_sys::Object::new();
    js_sys::Reflect::set(&track_opts, &"color".into(), &"#ff5f5f".into()).ok();
    js_sys::Reflect::set(&track_opts, &"weight".into(), &JsValue::from_f64(1.5)).ok();
    js_sys::Reflect::set(&track_opts, &"opacity".into(), &JsValue::from_f64(0.45)).ok();
    js_sys::Reflect::set(&track_opts, &"dashArray".into(), &"6, 8".into()).ok();
    let track = create_polyline(&track_pts.into(), &track_opts.into());
    track.polyline_add_to(map);

    let mut pins = Vec::with_capacity(t.route.len());
    for (i, (lat, lng)) in t.route.iter().enumerate() {
        let pin_opts = js_sys::Object::new();
        js_sys::Reflect::set(
            &pin_opts,
            &"icon".into(),
            &JsValue::from(html_icon_at("<div class=\"wp-pin\"></div>", "waypoint-div-icon", 16.0, 8.0, 16.0)),
        ).ok();
        let pin = create_marker(&lat_lng(*lat, *lng), &pin_opts.into());
        pin.bind_popup(&format!("WP {}", i + 1));
        pin.marker_add_to(map);
        pins.push(pin);
    }
    TheaterLayers { aor, track, pins }
}

/// Fire an impact burst at (lat, lng). Lives in IMPACT_PANE (below markers),
/// self-destructs after the keyframe completes.
fn fire_burst(map: &Map, lat: f64, lng: f64, hit: bool) {
    let (core, mid) = if hit { ("#ff2d2d", "#ffae2b") } else { ("#dddddd", "#888888") };
    let html = format!(
        "<div class=\"impact-burst{miss}\" style=\"--blast-core:{core};--blast-mid:{mid};\">{svg}</div>",
        miss = if hit { "" } else { " miss" },
        svg = inline_svg(EXPLOSION_SVG),
    );
    let opts = js_sys::Object::new();
    js_sys::Reflect::set(&opts, &"icon".into(), &JsValue::from(html_icon(&html, "impact-div-icon", IMPACT_SIZE))).ok();
    // The pane option is what puts the burst UNDER the airframes.
    js_sys::Reflect::set(&opts, &"pane".into(), &IMPACT_PANE.into()).ok();
    js_sys::Reflect::set(&opts, &"interactive".into(), &JsValue::FALSE).ok();
    js_sys::Reflect::set(&opts, &"keyboard".into(), &JsValue::FALSE).ok();
    let m = create_marker(&lat_lng(lat, lng), &opts.into());
    m.marker_add_to(map);
    let cleanup = Closure::once(Box::new(move || m.marker_remove()) as Box<dyn FnOnce()>);
    if let Some(w) = web_sys::window() {
        let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(cleanup.as_ref().unchecked_ref(), IMPACT_TTL_MS);
    }
    cleanup.forget();
}

/// Which theater the SERVER says the convoy is in: nearest AOR centre to the
/// drones' mean reported position. Exact for these six theaters (they are
/// thousands of km apart) and needs no wire change. `None` until fixes exist.
fn flown_theater(drones: &std::collections::HashMap<uuid::Uuid, crate::state::DroneState>) -> Option<TheaterId> {
    if drones.is_empty() { return None; }
    let n = drones.len() as f64;
    let (mlat, mlng) = drones.values().fold((0.0, 0.0), |(a, b), d| (a + d.position.latitude / n, b + d.position.longitude / n));
    TheaterId::ALL.iter().copied().min_by(|x, y| {
        let d = |t: &TheaterId| { let c = t.theater().center; (c.0 - mlat).powi(2) + (c.1 - mlng).powi(2) };
        d(x).partial_cmp(&d(y)).unwrap_or(std::cmp::Ordering::Equal)
    })
}

/// Offset a point by `km` along `heading_deg` -- the shot lands where the
/// drone was aiming, not on top of it. Flat-earth is fine at these ranges.
fn project(lat: f64, lng: f64, heading_deg: f64, km: f64) -> (f64, f64) {
    let h = heading_deg.to_radians();
    let dlat = (km * h.cos()) / 111.0;
    let dlng = (km * h.sin()) / (111.0 * lat.to_radians().cos().max(0.2));
    (lat + dlat, lng + dlng)
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

/// Tactical map panel
#[component]
pub fn MapPanel() -> impl IntoView {
    let state = use_app_state();
    let map_id = "tactical-map";

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

            // Create map on the initially selected theater
            let initial = state.selected_theater.get_untracked().theater();
            // Rc: the handle is shared by the marker sync, the theater-switch
            // effect and the burst effect. wasm_bindgen extern types have no
            // own Clone (a .clone() derefs to JsValue and loses the type), so
            // Rc is the way to hand one Map to several closures.
            let map: Rc<Map> = Rc::new(create_map(map_id));
            map.set_view(&lat_lng(initial.center.0, initial.center.1), initial.zoom);

            // Impact pane: created once, sits under markerPane (600).
            let pane = map.create_pane(IMPACT_PANE);
            let _ = pane.style().set_property("z-index", IMPACT_PANE_Z);
            let _ = pane.style().set_property("pointer-events", "none");

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

            // Theater layers (AOR ring, track, pins) live in one handle so a
            // switch clears the whole set and redraws.
            let layers: Rc<RefCell<Option<TheaterLayers>>> =
                Rc::new(RefCell::new(Some(draw_theater(&map, initial))));
            // The active route the flight loop follows; swapped on theater change.
            let route: Rc<Cell<&'static [(f64, f64)]>> = Rc::new(Cell::new(initial.route));

            // ---------------------------------------------------------------
            // Convoy: one marker per drone, flying the route line astern.
            //
            // Markers are created INCREMENTALLY. At mount the drones map is
            // empty — state fills from the 2s poll — so a one-shot snapshot
            // here would put zero airframes on the map forever. A sync
            // interval watches state and adds a marker for every callsign it
            // hasn't seen; the flight loop animates whatever exists.
            // ---------------------------------------------------------------
            let markers: Rc<RefCell<Vec<(String, Marker)>>> = Rc::new(RefCell::new(Vec::new()));

            let sync = {
                let markers = Rc::clone(&markers);
                let route = Rc::clone(&route);
                let map = Rc::clone(&map);
                move || {
                    let drones = state.drones.get_untracked();
                    let mut ordered: Vec<_> = drones.values().cloned().collect();
                    // Stable order, so a drone keeps its slot in the formation
                    // instead of shuffling with HashMap iteration order.
                    ordered.sort_by(|a, b| a.callsign.cmp(&b.callsign));

                    let mut known = markers.borrow_mut();
                    let live = state.live_positions.get_untracked();
                    for drone in &ordered {
                        let accent = status_accent(&drone.status);
                        // Popup carries the SAME live ALT/HDG the bottom-right
                        // readout shows, so a clicked airframe and the readout
                        // can never disagree.
                        let (alt, hdg) = live
                            .get(&drone.drone_id)
                            .map(|p| (p.altitude_m, p.heading_deg))
                            .unwrap_or((drone.position.altitude_m, drone.position.heading_deg));
                        let popup = format!(
                            "<div style='font-family: monospace; color: #00ff41; background: #0a0f0d; \
                             padding: 8px; border: 1px solid #00ff41; white-space: nowrap;'>\
                             <b>{}</b>\
                             <br/><span style='color:#557755;'>FUEL:</span> {:.0}%\
                             <br/><span style='color:#557755;'>ACC:</span>  {:.1}%\
                             <br/><span style='color:#557755;'>ALT:</span>  {:.0} m\
                             <br/><span style='color:#557755;'>HDG:</span>  {:03.0}°</div>",
                            drone.callsign, drone.fuel_pct, drone.accuracy_pct, alt, hdg
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

                        let ip = route.get()[0];
                        let marker = create_marker(&lat_lng(ip.0, ip.1), &opts.into());
                        marker.bind_popup(&popup);
                        marker.marker_add_to(&map);

                        // Clicking an airframe SELECTS it app-wide -- the
                        // bottom-right readout switches to it and its card
                        // highlights -- so the map, the readout and the cards
                        // are one selection, not three. Toggle on re-click.
                        {
                            let id = drone.drone_id;
                            let on_click = Closure::wrap(Box::new(move || {
                                state.selected_drone.update(|sel| {
                                    *sel = if *sel == Some(id) { None } else { Some(id) };
                                });
                            }) as Box<dyn Fn()>);
                            marker.marker_on("click", on_click.as_ref().unchecked_ref());
                            on_click.forget(); // lives as long as the marker
                        }

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
            // Flight loop -- SERVER-ANCHORED.
            //
            // The airframes fly the positions the API reports, not a client
            // route. Each poll (2 s) delivers a fresh fix per drone; the loop
            // interpolates from the previous fix to the latest over one poll
            // interval, so a marker sits EXACTLY on the server fix at every
            // poll boundary and glides between them at animation rate. That
            // is what makes the map, the GPS readout on the cards, and the
            // database agree: one truth (the simulator flying the theater
            // route), displayed smoothly.
            //
            // `positions` (last drawn lat/lng/heading per callsign) still feeds
            // the impact bursts. `live_positions` in AppState feeds the cards.
            // ---------------------------------------------------------------
            /// Per-drone interpolation state: (prev fix, latest fix, when the
            /// latest arrived in ms, drone_id).
            type Fix = (f64, f64, f64, f32); // lat, lng, alt, hdg
            let anchors: Rc<RefCell<std::collections::HashMap<String, (Fix, Fix, f64, uuid::Uuid)>>> =
                Rc::new(RefCell::new(std::collections::HashMap::new()));
            let positions: Rc<RefCell<Vec<(String, f64, f64, f64)>>> = Rc::new(RefCell::new(Vec::new()));

            // Anchor updater: on every state.drones change (each poll), shift
            // latest -> prev and store the new server fix with its arrival time.
            {
                let anchors = Rc::clone(&anchors);
                Effect::new(move |_| {
                    let drones = state.drones.get();
                    let now = js_sys::Date::now();
                    let mut a = anchors.borrow_mut();
                    for d in drones.values() {
                        let fix: Fix = (d.position.latitude, d.position.longitude, d.position.altitude_m, d.position.heading_deg);
                        match a.get_mut(&d.callsign) {
                            Some((prev, latest, at, _)) => {
                                if *latest != fix { *prev = *latest; *latest = fix; *at = now; }
                            }
                            None => { a.insert(d.callsign.clone(), (fix, fix, now, d.drone_id)); }
                        }
                    }
                });
            }

            let tick = {
                let markers = Rc::clone(&markers);
                let anchors = Rc::clone(&anchors);
                let positions = Rc::clone(&positions);
                move || {
                    let now = js_sys::Date::now();
                    let a = anchors.borrow();
                    let mut pos = positions.borrow_mut();
                    pos.clear();
                    let mut live: std::collections::HashMap<uuid::Uuid, crate::state::LivePosition> =
                        std::collections::HashMap::new();

                    for (callsign, marker) in markers.borrow().iter() {
                        let Some(((plat, plng, palt, phdg), (lat1, lng1, alt1, hdg1), at, id)) = a.get(callsign) else { continue };
                        // Fraction of the way from prev to latest, by wall clock
                        // over one poll interval; clamps at the latest fix if a
                        // poll is late (never extrapolates past ground truth).
                        let f = ((now - at) / f64::from(POLL_INTERVAL_MS)).clamp(0.0, 1.0);
                        let lat = plat + (lat1 - plat) * f;
                        let lng = plng + (lng1 - plng) * f;
                        let alt = palt + (alt1 - palt) * f;
                        // Heading: shortest-arc blend, then snap to the latest.
                        let dh = ((hdg1 - phdg + 540.0) % 360.0) - 180.0;
                        let heading = ((phdg + dh * f as f32) + 360.0) % 360.0;

                        marker.set_lat_lng(&lat_lng(lat, lng));
                        pos.push((callsign.clone(), lat, lng, f64::from(heading)));
                        live.insert(*id, crate::state::LivePosition { latitude: lat, longitude: lng, altitude_m: alt, heading_deg: heading });

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
                    drop(pos);
                    drop(a);
                    // Publish the smoothed fixes for the GPS readout.
                    state.live_positions.set(live);
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

            // ---------------------------------------------------------------
            // Theater switch. Reactive on the header selector: re-centre,
            // drop the old AOR/track/pins, draw the new set, hand the flight
            // loop the new route and restart the convoy at its IP.
            // ---------------------------------------------------------------
            {
                let map = Rc::clone(&map);
                let layers = Rc::clone(&layers);
                let route = Rc::clone(&route);
                let markers = Rc::clone(&markers);
                let last: Rc<Cell<TheaterId>> = Rc::new(Cell::new(state.selected_theater.get_untracked()));
                Effect::new(move |_| {
                    let id = state.selected_theater.get();
                    if id == last.get() { return; }
                    last.set(id);
                    let t = id.theater();
                    map.set_view(&lat_lng(t.center.0, t.center.1), t.zoom);
                    if let Some(old) = layers.borrow_mut().take() { old.clear(); }
                    *layers.borrow_mut() = Some(draw_theater(&map, t));
                    route.set(t.route);
                    // Airframes fly SERVER positions. Selecting a theater here
                    // changes what the map shows; the convoy's real track comes
                    // from the simulator's --theater. If they differ, the map
                    // overlay says so (see the SIM banner) rather than faking
                    // airframes onto pins the simulator is not flying.
                    log::info!("map: theater -> {} ({} waypoints)", t.label, t.route.len());
                });
            }

            // ---------------------------------------------------------------
            // Impact bursts. Every NEW engagement in the feed fires one burst
            // from the shooting drone's current map position, projected a few
            // km along its heading (the shot goes where the drone is aiming,
            // not on top of it). Engagements are not tied to waypoints in the
            // simulator -- they happen mid-sortie -- so this is the honest
            // place for them. Seen-set prevents refiring on every poll.
            // ---------------------------------------------------------------
            {
                let map = Rc::clone(&map);
                let positions = Rc::clone(&positions);
                let seen: Rc<RefCell<HashSet<uuid::Uuid>>> = Rc::new(RefCell::new(HashSet::new()));
                let primed = Rc::new(Cell::new(false));
                Effect::new(move |_| {
                    let events = state.engagements.get();
                    let mut seen = seen.borrow_mut();
                    // First observation: mark everything seen without firing,
                    // so a page load mid-mission doesn't detonate 20 bursts.
                    if !primed.get() {
                        for e in &events { seen.insert(e.id); }
                        primed.set(true);
                        return;
                    }
                    let pos = positions.borrow();
                    // Collect first, then insert: filtering on `seen` while
                    // inserting into it is a simultaneous shared+mutable borrow.
                    let fresh: Vec<_> = events.iter().filter(|e| !seen.contains(&e.id)).cloned().collect();
                    for e in &fresh {
                        seen.insert(e.id);
                        if let Some((_, lat, lng, hdg)) = pos.iter().find(|(cs, ..)| cs == &e.callsign) {
                            // 6-14 km ahead of the airframe, hashed off the id so
                            // simultaneous shots don't stack on one pixel.
                            let spread = (e.id.as_u128() % 9) as f64;
                            let (blat, blng) = project(*lat, *lng, *hdg + (spread - 4.0) * 6.0, 6.0 + spread);
                            fire_burst(&map, blat, blng, e.hit);
                        }
                    }
                    if seen.len() > 400 { seen.clear(); }
                });
            }

            log::info!("map ready: {} — airframes join as they register on a {}-waypoint route",
                       initial.label, initial.route.len());
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

    // The ALT/HDG readout tracks the SELECTED drone, or the convoy LEAD when
    // nothing is selected -- so it is never blank. Values come from the
    // smoothed live fixes (same source as the GPS row and airframe heading),
    // falling back to the last polled position before the first fix lands.
    // Returns (callsign, altitude_m, heading_deg).
    let readout = move || -> Option<(String, f64, f32)> {
        let drones = state.drones.get();
        let live = state.live_positions.get();
        let target = selected_drone().and_then(|id| drones.get(&id).cloned()).or_else(|| {
            // Lead = lowest callsign (ALPHA-01); stable regardless of HashMap order.
            drones.values().min_by(|a, b| a.callsign.cmp(&b.callsign)).cloned()
        })?;
        let (alt, hdg) = live
            .get(&target.drone_id)
            .map(|p| (p.altitude_m, p.heading_deg))
            .unwrap_or((target.position.altitude_m, target.position.heading_deg));
        Some((target.callsign, alt, hdg))
    };

    view! {
        <div class="map-container">
            <div id=map_id class="leaflet-map"></div>

            <div class="map-overlay">
                // ONE HUD strip. Segment 1: the AOR being viewed. Segment 2
                // (only when the sim is flying elsewhere): the truth-guard
                // warning with the exact command. One dark bar, no stacking.
                <div class="map-hud">
                    <div class="map-hud-row aor">
                        <span class="status-dot nominal"></span>
                        {move || state.selected_theater.get().theater().aor}
                    </div>
                    // Tasking in flight: the selector issued an order and the
                    // convoy has not reported from the new theater yet. Purely
                    // a transition state -- clears itself when the server's
                    // positions land in the viewed theater. No operator
                    // action is ever required from here: the UI is the
                    // commander, the record is the truth, the sim obeys.
                    {move || {
                        let viewed = state.selected_theater.get();
                        let flown = flown_theater(&state.drones.get());
                        if flown == Some(viewed) && state.retasking.get_untracked().is_some() {
                            state.retasking.set(None);
                        }
                        let pending = state.retasking.get().is_some()
                            || matches!(flown, Some(f) if f != viewed);
                        let label = match flown {
                            Some(f) if f != viewed => format!("RETASKING FROM {}", f.theater().label),
                            _ => "RETASKING — AWAITING CONVOY".to_string(),
                        };
                        pending.then(|| view! {
                            <div class="map-hud-row retask">
                                <span class="status-dot warning pulse"></span>
                                {label}
                            </div>
                        })
                    }}
                    // A rejected tasking order is shown, not swallowed.
                    {move || state.retask_error.get().map(|e| view! {
                        <div class="map-hud-row error">
                            <span class="status-dot critical"></span>
                            {format!("TASKING REJECTED: {e}")}
                        </div>
                    })}
                </div>
                {move || drone_position().map(|pos| view! {
                    <div class="map-control">
                        <span class="text-accent">"SEL:"</span>
                        {format!("{:.4}°N {:.4}°E", pos.latitude, pos.longitude)}
                    </div>
                })}
            </div>

            <div style="position: absolute; bottom: 16px; right: 16px; z-index: 1000;">
                <div class="map-control readout" style="font-size: 0.7rem;">
                    <span class="readout-cs">
                        {move || readout().map(|(cs, _, _)| cs).unwrap_or_else(|| "NO CONTACT".to_string())}
                    </span>
                    <span class="text-muted">"ALT"</span>
                    <span class="readout-val">
                        {move || readout().map(|(_, alt, _)| format!("{:>5.0} m", alt)).unwrap_or_else(|| "  --- m".to_string())}
                    </span>
                    <span class="text-muted">"HDG"</span>
                    <span class="readout-val">
                        {move || readout().map(|(_, _, hdg)| format!("{:03.0}°", hdg)).unwrap_or_else(|| "---°".to_string())}
                    </span>
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
