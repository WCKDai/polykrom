//! Every registered command stays reachable from the shell (PK-90.shell): on a ribbon group, as a
//! button or in the group's drop-down, or through command search; the entries that left the title
//! bar (the start page, File, Help, Preferences, Keyboard Shortcuts, Report a bug, the version)
//! are in the application menu, the Help menu or the status bar; the workspaces menu still opens;
//! every shell panel is drawn.

use std::collections::BTreeSet;

use polykrom_ui_egui::scenario::{Harness, handle_at, rect_of};
use serde_json::json;

const TABS: [&str; 10] = ["MODEL", "SURFACE", "ASSEMBLE", "INSERT", "INSPECT", "MANAGE", "VIEW", "SHEET METAL", "SKETCH", "RENDER"];

#[test]
fn every_registered_command_is_on_a_ribbon_group_or_found_by_search() {
    let h = Harness::new();
    let on_ribbon: BTreeSet<&str> = TABS
        .iter()
        .flat_map(|tab| polykrom_ui_egui::toolbar_custom::panel_lists(tab))
        .flat_map(|(_, cmds, _)| cmds.into_iter().map(|c| c.id))
        .collect();
    let mut missing = Vec::new();
    let mut counted = 0;
    for spec in polykrom_engine::command_specs() {
        if polykrom_ui_egui::workspace::AGENT_ONLY.contains(&spec.id) {
            continue;
        }
        counted += 1;
        let searched = |query: &str| polykrom_ui_egui::shortcut_box::search(&h.app, query).iter().any(|row| row.0 == spec.id);
        if !(on_ribbon.contains(spec.id) || searched(spec.label) || searched(spec.id)) {
            missing.push(format!("{} ({})", spec.id, spec.label));
        }
    }
    assert!(counted > 200, "{counted} commands checked");
    assert!(missing.is_empty(), "{} commands are neither on a ribbon group nor found by search:\n{}", missing.len(), missing.join("\n"));
}

#[test]
fn the_entries_that_left_the_title_bar_stay_reachable_and_every_panel_is_drawn() {
    for layout in ["floating", "docked"] {
        let mut h = Harness::new();
        h.app.home.open = false;
        h.call("ui.set", json!({"layout": layout}));
        h.app.run("solid.box", json!({"length": 10, "width": 10, "height": 10})).unwrap();
        h.frames(3);
        // The panels of view 01.
        for name in ["appbar:logo", "appbar:document", "titlebar:save", "titlebar:export", "appbar:undo", "appbar:redo", "ribbon:search"] {
            assert!(rect_of(name).is_some(), "{layout}: {name} in the title bar");
        }
        for name in ["toolbar", "browser:card", "browser:filter", "browser:identity", "documents:strip", "view:bar", "status:bar", "history:end"] {
            assert!(rect_of(name).is_some(), "{layout}: {name} is drawn");
        }
        for name in ["tab:MODEL", "tab:RENDER", "tab:design:0", "tab:new", "timeline:Box1", "history:marker"] {
            assert!(handle_at(name).is_some() || rect_of(name).is_some(), "{layout}: {name} is drawn");
        }
        // The status bar: the start page, Preferences, Help, Report a bug and the version.
        for name in ["titlebar:home", "titlebar:preferences", "appbar:help", "titlebar:report", "titlebar:version", "status:credit", "status:units"] {
            let r = rect_of(name).unwrap_or_else(|| panic!("{layout}: {name}"));
            assert!(rect_of("status:bar").unwrap().contains_rect(r), "{layout}: {name} is in the status bar");
        }
        // The Help menu.
        h.step(&json!({"click": {"handle": "appbar:help"}})).unwrap();
        for item in ["Documentation", "About Polykrom", "Keyboard Shortcuts", "Messages…", "Report an Issue…", "Crash Reports…"] {
            assert!(h.widget(item).is_some(), "{layout}: {item} in the Help menu");
        }
        h.step(&json!({"key": "Escape"})).unwrap();
        h.frames(2);
        // The application menu, from the logo.
        h.step(&json!({"click": {"handle": "appbar:file"}})).unwrap();
        for item in [
            "Start page",
            "New Design",
            "Open…",
            "Open Recent",
            "Insert STEP…",
            "Insert Mesh…",
            "Insert Design…",
            "Save",
            "Save As…",
            "Save As JSON…",
            "Export STL…",
            "Export 3MF…",
            "Export STEP…",
            "Export Current View as DXF…",
            "Export OBJ…",
            "Export URDF…",
            "Export Design as JSON…",
            "Design Properties…",
            "Workspace…",
            "Preferences…",
            "Keyboard Shortcuts…",
            "Help",
            "Quit",
        ] {
            assert!(h.widget(item).is_some(), "{layout}: {item} in the application menu");
        }
        h.step(&json!({"widget": "Keyboard Shortcuts…"})).unwrap();
        // (The shortcut editor is the Preferences window's Keyboard page.)
        assert!(h.app.prefs_window.open, "{layout}: Keyboard Shortcuts opens from the application menu");
        h.call("ui.prefs", json!({"button": "cancel"}));
        h.frames(2);
        h.step(&json!({"click": {"handle": "appbar:file"}})).unwrap();
        h.step(&json!({"widget": "Workspace…"})).unwrap();
        assert!(h.app.viewport.context_menu.is_some(), "{layout}: the workspaces menu opens");
        for slug in ["design", "render", "animation", "simulation", "manufacture", "drawing"] {
            assert!(rect_of(&format!("workspace:menu:{slug}")).is_some(), "{layout}: workspace {slug} listed");
        }
        h.step(&json!({"key": "Escape"})).unwrap();
        h.frames(2);
        // No tab for a workspace without a backend.
        for tab in ["DRAWING", "MANUFACTURE", "AUTOMATE", "ANIMATION", "SIMULATION"] {
            assert!(handle_at(&format!("tab:{tab}")).is_none(), "{layout}: no {tab} tab");
        }
    }
}
