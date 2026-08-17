//! # TacticalSelect
//!
//! A web-native dropdown in the HUD's own visual language. Deliberately NOT a
//! `<select>`: browsers hand that element to the OS, and on macOS you get a
//! Cocoa popup that ignores every CSS rule here. This is a button plus a
//! panel, fully styled, keyboard-navigable (Enter/Space open, ↑↓ move,
//! Enter selects, Esc closes), closes on outside click, and exposes the ARIA
//! listbox pattern so it is a real control, not a div that looks like one.
//!
//! Generic over the option key so the mission selector, and any later
//! dropdown, share one implementation.

use leptos::prelude::*;
use leptos::ev;
use wasm_bindgen::JsCast;

/// One selectable option: a stable key and its display label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectOption<K: Copy + PartialEq + 'static> {
    pub key: K,
    pub label: &'static str,
}

/// Visual accent for the control. Danger reads as a mode switch (red); accent
/// is the standard HUD green.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectTone {
    Accent,
    Danger,
}

impl SelectTone {
    fn class(self) -> &'static str {
        match self {
            SelectTone::Accent => "tone-accent",
            SelectTone::Danger => "tone-danger",
        }
    }
}

#[component]
pub fn TacticalSelect<K>(
    /// Small caps label above the current value, e.g. "THEATER".
    #[prop(into)] label: String,
    /// The options, in display order.
    options: Vec<SelectOption<K>>,
    /// Currently selected key.
    value: RwSignal<K>,
    #[prop(default = SelectTone::Accent)] tone: SelectTone,
) -> impl IntoView
where
    K: Copy + PartialEq + Send + Sync + 'static,
{
    let open = RwSignal::new(false);
    // Keyboard cursor while open; -1 = follow the selected value.
    let cursor = RwSignal::new(-1_i32);
    let opts = StoredValue::new(options);
    let root: NodeRef<leptos::html::Div> = NodeRef::new();

    let current_label = move || {
        let v = value.get();
        opts.with_value(|o| o.iter().find(|x| x.key == v).map(|x| x.label).unwrap_or("—"))
    };
    let selected_index = move || {
        let v = value.get_untracked();
        opts.with_value(|o| o.iter().position(|x| x.key == v).map(|i| i as i32).unwrap_or(0))
    };

    let choose = move |k: K| {
        value.set(k);
        open.set(false);
        cursor.set(-1);
    };

    let toggle = move |_| {
        let now_open = !open.get_untracked();
        open.set(now_open);
        cursor.set(if now_open { selected_index() } else { -1 });
    };

    // Close on outside click: a document-level listener that checks whether
    // the event target lives inside our root node.
    Effect::new(move |_| {
        if !open.get() {
            return;
        }
        let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
        let handler = wasm_bindgen::closure::Closure::wrap(Box::new(move |e: web_sys::MouseEvent| {
            let inside = e
                .target()
                .and_then(|t| t.dyn_into::<web_sys::Node>().ok())
                .and_then(|n| root.get_untracked().map(|r| r.contains(Some(&n))))
                .unwrap_or(false);
            if !inside {
                open.set(false);
                cursor.set(-1);
            }
        }) as Box<dyn Fn(web_sys::MouseEvent)>);
        let _ = document.add_event_listener_with_callback("mousedown", handler.as_ref().unchecked_ref());
        // Intentionally leaked per open cycle: the panel is short-lived and
        // the listener is idempotent-safe (it only ever closes).
        handler.forget();
    });

    let on_key = move |e: ev::KeyboardEvent| {
        let n = opts.with_value(|o| o.len() as i32);
        match e.key().as_str() {
            "Enter" | " " => {
                e.prevent_default();
                if open.get_untracked() {
                    let i = cursor.get_untracked().max(0);
                    if let Some(k) = opts.with_value(|o| o.get(i as usize).map(|x| x.key)) {
                        choose(k);
                    }
                } else {
                    open.set(true);
                    cursor.set(selected_index());
                }
            }
            "ArrowDown" => {
                e.prevent_default();
                if !open.get_untracked() { open.set(true); cursor.set(selected_index()); }
                else { cursor.update(|c| *c = (*c + 1).rem_euclid(n)); }
            }
            "ArrowUp" => {
                e.prevent_default();
                if !open.get_untracked() { open.set(true); cursor.set(selected_index()); }
                else { cursor.update(|c| *c = (*c - 1).rem_euclid(n)); }
            }
            "Escape" => { open.set(false); cursor.set(-1); }
            _ => {}
        }
    };

    view! {
        <div class=format!("tsel {}", tone.class()) node_ref=root>
            <span class="tsel-label">{label}</span>
            <button
                type="button"
                class="tsel-button"
                aria-haspopup="listbox"
                aria-expanded=move || open.get().to_string()
                on:click=toggle
                on:keydown=on_key
            >
                <span class="tsel-value">{current_label}</span>
                <span class="tsel-caret" aria-hidden="true">
                    <svg viewBox="0 0 10 6" width="10" height="6">
                        <path d="M1 1 L5 5 L9 1" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>
                    </svg>
                </span>
            </button>
            {move || open.get().then(|| {
                let items = opts.get_value();
                view! {
                    <ul class="tsel-panel" role="listbox">
                        {items.into_iter().enumerate().map(|(i, o)| {
                            let is_selected = move || value.get() == o.key;
                            let is_cursor = move || cursor.get() == i as i32;
                            view! {
                                <li
                                    class="tsel-option"
                                    class:selected=is_selected
                                    class:cursor=is_cursor
                                    role="option"
                                    aria-selected=move || is_selected().to_string()
                                    on:mouseenter=move |_| cursor.set(i as i32)
                                    on:click=move |_| choose(o.key)
                                >
                                    <span class="tsel-tick" aria-hidden="true"></span>
                                    {o.label}
                                </li>
                            }
                        }).collect_view()}
                    </ul>
                }
            })}
        </div>
    }
}
