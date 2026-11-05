use super::*;
use crate::scenario::{Harness, handle_at, rect_of};
use crate::shell::{LayoutMode, SHELL_GAP};

#[test]
fn browser_body_clicks_fill_and_toggle_save_mesh_inputs() {
    for mode in [LayoutMode::Floating, LayoutMode::Docked] {
        let mut h = Harness::new();
        h.app.ui.layout = mode;
        for (name, x) in [("One", 0), ("Two", 20)] {
            h.app.run("solid.box", json!({"length":10,"width":10,"height":10,"corner":[x,0,0],"operation":"new_body","body_name":name})).unwrap();
        }
        h.app.start("file.save_mesh");
        h.settle();
        for (clicked, expected) in [("One", vec!["One"]), ("Two", vec!["One", "Two"]), ("One", vec!["Two"])] {
            h.step(&json!({"click":{"handle":format!("row:{clicked}")}})).unwrap();
            let d = h.app.dialog.as_ref().unwrap();
            let crate::dialogs::Kind::SaveMesh(k) = &d.kind else { panic!("Save as Mesh is open") };
            let commands = crate::dialogs_save_mesh::commands(k, &d.inputs).unwrap();
            assert_eq!(commands[0].1["bodies"], json!(expected), "{mode:?}: clicked {clicked}");
            assert!(crate::dialogs_save_mesh::ready(&d.inputs));
            assert!(h.app.session.selection.is_empty(), "dialog picks do not change the global selection");
        }
        h.step(&json!({"click":{"handle":"row:Two"}})).unwrap();
        assert!(!crate::dialogs_save_mesh::ready(&h.app.dialog.as_ref().unwrap().inputs));
        h.step(&json!({"key":"Escape"})).unwrap();
        h.step(&json!({"click":{"handle":"row:One"}})).unwrap();
        assert_eq!(h.app.session.selection, vec![Sel::Body { name: "One".into() }]);
    }
}

#[test]
fn browser_body_clicks_respect_the_active_dialog_input() {
    let mut h = Harness::new();
    for (name, x) in [("One", 0), ("Two", 20)] {
        h.app.run("solid.box", json!({"length":10,"width":10,"height":10,"corner":[x,0,0],"operation":"new_body","body_name":name})).unwrap();
    }
    h.app.start("solid.combine");
    h.app.dialog.as_mut().unwrap().preview_enabled = false;
    h.settle();
    h.step(&json!({"click":{"handle":"row:One"}})).unwrap();
    assert_eq!(h.app.dialog.as_ref().unwrap().active, 1, "a target pick advances to Tool Bodies");
    h.step(&json!({"click":{"handle":"row:Two"}})).unwrap();
    let d = h.app.dialog.as_ref().unwrap();
    assert_eq!(d.inputs[0].items, vec![Sel::Body { name: "One".into() }]);
    assert_eq!(d.inputs[1].items, vec![Sel::Body { name: "Two".into() }]);
    h.call("ui.dialogInput", json!({"index":0}));
    h.step(&json!({"click":{"handle":"row:Two"}})).unwrap();
    assert_eq!(h.app.dialog.as_ref().unwrap().inputs[0].items, vec![Sel::Body { name: "Two".into() }], "a single input replaces its pick");
    h.step(&json!({"key":"Escape"})).unwrap();
    h.app.start("solid.fillet");
    h.settle();
    h.step(&json!({"click":{"handle":"row:One"}})).unwrap();
    assert!(h.app.dialog.as_ref().unwrap().inputs.iter().all(|i| i.items.is_empty()), "a body is not an edge");
    assert!(h.app.session.selection.is_empty(), "an incompatible pick does not leak into the global selection");
}

/// The Design Browser is a full-height column from under the prompt row to the history
/// card; its rows scroll inside it, and folding it leaves its header.
#[test]
fn browser_card_fills_its_column_and_scrolls_without_covering_history() {
    let mut h = Harness::new();
    h.call("ui.resize", json!({"width":1400,"height":900}));
    h.settle();
    let short = rect_of("browser:card").unwrap();
    let history = rect_of("history:card").unwrap();
    assert_eq!(short.left(), SHELL_GAP);
    assert_eq!(short.width(), 250.0);
    // The AI review's prompt row sits under the ribbon card; the Browser card starts under it.
    let prompt = rect_of("review:prompt_row").unwrap();
    assert_eq!(prompt.top(), rect_of("ribbon:card").unwrap().bottom() + SHELL_GAP);
    assert_eq!(short.top(), prompt.bottom() + SHELL_GAP);
    assert!((short.bottom() - (history.top() - SHELL_GAP)).abs() < 1.0, "full height: {short:?} {history:?}");
    assert!(drawn_rows().iter().all(|(_, rect, _, _)| rect.height() == 26.0));

    for i in 0..35 {
        h.app.run("sketch.create", json!({"plane":"XY", "name":format!("Sketch{i}")})).unwrap();
        h.app.run("sketch.finish", json!({})).unwrap();
    }
    h.settle();
    let tall = rect_of("browser:card").unwrap();
    assert_eq!(tall, short, "the column keeps its height whatever it lists");
    // The browser is a full-height column at the left; the scene sits flush to its right.
    assert_eq!(h.app.viewport.rect.unwrap().left(), tall.right());
    assert!(tall.bottom() <= h.app.viewport.rect.unwrap().bottom());
    assert!(handle_at("row:Sketch34").is_none(), "last row starts below the scroll clip");
    let camera = h.app.cam;
    h.call("ui.scroll", json!({"x":tall.center().x,"y":tall.center().y,"delta":-1800}));
    h.settle();
    assert!(handle_at("row:Sketch34").is_some(), "scrolling must reveal the last row");
    assert_eq!(h.app.cam, camera);
    assert_eq!(rect_of("browser:card").unwrap(), tall);

    h.call("ui.set", json!({"layout":"docked"}));
    h.settle();
    // Docked, the column hugs the window's left edge; the viewport spans the width behind it and
    // keeps its model clear of the column through `covered`.
    assert_eq!(rect_of("browser:card").unwrap().left(), 0.0);
    assert_eq!(h.app.viewport.rect.unwrap().left(), 0.0);
    assert!(drawn_rows().iter().all(|(_, rect, _, _)| rect.height() == 26.0));
    h.call("ui.set", json!({"layout":"floating"}));
    // Folding the design's root empties the tree but the column keeps its full height; folding
    // the card itself to its header is what shrinks it.
    h.app.tree.collapsed.insert("c0".into());
    h.settle();
    assert_eq!(rect_of("browser:card").unwrap().height(), short.height());
    h.app.tree.panel_collapsed = true;
    h.settle();
    assert!(rect_of("browser:card").unwrap().height() < short.height() / 2.0);
}

#[test]
fn selected_row_paints_tab_soft_fill_and_text_colour_in_both_layouts() {
    let previous = crate::theme::is_dark();
    for dark in [false, true] {
        crate::theme::set_dark(dark);
        for mode in [LayoutMode::Floating, LayoutMode::Docked] {
            for tab in ["MODEL", "SKETCH", "SURFACE", "MESH", "FORM", "SHEET METAL", "RENDER"] {
                let ctx = crate::fonts::test_context();
                let mut app = SolveApp::new(Default::default(), Default::default());
                app.ui.layout = mode;
                app.ui.tab = tab.into();
                crate::theme::TabAccent::publish(&app, &ctx);
                let accent = crate::theme::TabAccent::for_tab(&Tokens::get(), tab);
                let mut row_rect = Rect::NOTHING;
                let out = crate::test_support::frame(&ctx, egui::RawInput::default(), |ui| {
                    let row = Row { selected: true, label: "Selected body", dim: true, color: Some(Tokens::get().error), ..Default::default() };
                    row_rect = draw_row(ui, egui::Id::new("selected-row"), &row).rect.unwrap();
                });
                assert_eq!(row_rect.height(), 26.0);
                assert!(out.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == row_rect && r.fill == accent.soft)));
                // The label keeps the text colour on the tint (the kit's selected browser row).
                assert!(out.shapes.iter().any(
                    |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Selected body" && t.fallback_color == Tokens::get().text)
                ));
            }
        }
    }
    crate::theme::set_dark(previous);
}

#[test]
fn card_and_column_keep_body_eyes_menus_and_group_drops() {
    for mode in [LayoutMode::Floating, LayoutMode::Docked] {
        let mut h = Harness::new();
        h.app.ui.layout = mode;
        for (name, x) in [("One", 0), ("Two", 20)] {
            h.app.run("solid.box", json!({"length":10,"width":10,"height":10,"corner":[x,0,0],"operation":"new_body","body_name":name})).unwrap();
        }
        let group =
            h.app.run("browser.group", json!({"component":0,"folder":"bodies","items":["One"],"name":"Group"})).unwrap()["group"].as_u64().unwrap();
        h.settle();
        h.step(&json!({"click":{"handle":"eye:Two"}})).unwrap();
        assert!(h.app.session.visibility.hidden_bodies.iter().any(|name| name == "Two"));
        h.step(&json!({"click":{"handle":"eye:Two"}})).unwrap();
        assert!(!h.app.session.visibility.hidden_bodies.iter().any(|name| name == "Two"));
        h.step(&json!({"click":{"handle":"row:Two"},"button":"right"})).unwrap();
        assert!(matches!(&h.app.menu.target, crate::context_menu::Target::Body { name } if name == "Two"));
        h.step(&json!({"key":"Escape"})).unwrap();
        h.step(&json!({"drag":[{"handle":"row:Two"},{"handle":"row:Group (1)"}]})).unwrap();
        h.settle();
        let group = h.app.session.doc.browser_groups.iter().find(|g| g.id == group).unwrap();
        assert!(group.items.iter().any(|name| name == "Two"), "body drop into the folder failed in {mode:?}");
    }
}

/// A second click on a Document Settings entry closes the window it opened, the way that
/// window's own × does, and a third opens it again (#4150): the Units menu, Preferences (a change
/// made there is put back), Parameters (a value typed but not applied is dropped) and Design
/// Properties (nothing typed there is saved).
#[test]
fn a_second_click_on_a_document_settings_entry_closes_its_window() {
    for mode in [LayoutMode::Floating, LayoutMode::Docked] {
        let mut h = Harness::new();
        h.app.ui.layout = mode;
        h.app.run("parameters.add", json!({"name":"width","expression":"10 mm"})).unwrap();
        h.settle();
        h.step(&json!({"click":{"chevron":"Document Settings"}})).unwrap();
        h.settle();
        // The Units, Precision and Parameters labels carry their values.
        let row = |start: &str| {
            let (_, rect, _, _) = drawn_rows().into_iter().find(|(label, ..)| label.starts_with(start)).unwrap();
            json!({"click":[rect.center().x, rect.center().y]})
        };

        h.step(&row("Units")).unwrap();
        assert!(h.app.viewport.context_menu.is_some() && h.app.menu.target == Target::Units, "{mode:?}: the first click opens the Units menu");
        h.step(&row("Units")).unwrap();
        h.frames(2);
        assert!(h.app.viewport.context_menu.is_none(), "{mode:?}: the second click closes it instead of reopening it");
        h.step(&row("Units")).unwrap();
        assert!(h.app.viewport.context_menu.is_some(), "{mode:?}: and a third opens it again");
        h.step(&json!({"key":"Escape"})).unwrap();

        h.step(&row("Precision")).unwrap();
        h.frames(2);
        assert!(h.app.prefs_window.open, "{mode:?}: the first click opens Preferences");
        let decimals = h.app.preferences.angle_decimals;
        h.app.preferences.angle_decimals = decimals + 2;
        h.step(&row("Precision")).unwrap();
        h.frames(2);
        assert!(!h.app.prefs_window.open, "{mode:?}: the second click closes it");
        assert_eq!(h.app.preferences.angle_decimals, decimals, "{mode:?}: and puts back what changed there, as × does");
        h.step(&row("Precision")).unwrap();
        h.frames(2);
        assert!(h.app.prefs_window.open, "{mode:?}: a third opens it again");
        h.step(&json!({"key":"Escape"})).unwrap();

        h.step(&row("Parameters")).unwrap();
        h.frames(2);
        assert!(h.app.params.is_some(), "{mode:?}: the first click opens Parameters");
        h.step(&json!({"click":{"handle":"param:width"}})).unwrap();
        h.step(&json!({"key":"A","cmd":true})).unwrap();
        h.step(&json!({"text":"25 mm"})).unwrap();
        h.step(&row("Parameters")).unwrap();
        h.frames(2);
        assert!(h.app.params.is_none(), "{mode:?}: the second click closes it");
        assert_eq!(h.app.session.doc.param("width").unwrap().expr, "10 mm", "{mode:?}: and drops a value typed but not applied");
        h.step(&row("Parameters")).unwrap();
        assert!(h.app.params.is_some(), "{mode:?}: a third opens it again");
        h.app.params = None;

        h.step(&row("Design Properties")).unwrap();
        assert!(h.app.design_properties.is_some(), "{mode:?}: the first click opens Design Properties");
        h.app.design_properties.as_mut().unwrap().fields[0] = "Typed".into();
        h.frames(2);
        h.step(&row("Design Properties")).unwrap();
        assert!(h.app.design_properties.is_none(), "{mode:?}: the second click closes it");
        assert_eq!(h.app.session.doc.metadata.project(), None, "{mode:?}: and saves nothing typed there");
        h.step(&row("Design Properties")).unwrap();
        assert!(h.app.design_properties.is_some(), "{mode:?}: a third opens it again");
    }
}

#[test]
fn browser_header_folds_the_floating_card_to_its_header_and_back() {
    let mut h = Harness::new();
    h.call("ui.resize", json!({"width":1400,"height":900}));
    h.settle();
    let open = rect_of("browser:card").unwrap();
    assert!(!h.app.tree.panel_collapsed);
    let at = handle_at("browser:header").unwrap();
    h.call("ui.click", json!({"x":at.x,"y":at.y}));
    h.settle();
    let folded = rect_of("browser:card").unwrap();
    assert!(h.app.tree.panel_collapsed);
    assert!(folded.height() < 60.0 && folded.height() < open.height(), "{folded:?} {open:?}");
    assert_eq!(folded.top(), open.top());
    assert!(drawn_rows().is_empty() || handle_at("row:Plate").is_none());
    h.call("ui.click", json!({"x":at.x,"y":at.y}));
    h.settle();
    assert!(!h.app.tree.panel_collapsed);
    assert!((rect_of("browser:card").unwrap().height() - open.height()).abs() < 1.0);
}

/// Tiny or odd window sizes never panic: the cards leave out what cannot fit.
#[test]
fn tiny_floating_windows_never_panic() {
    for (w, h) in [(100, 60), (240, 90), (320, 140), (700, 150), (700, 300), (1400, 120), (180, 800)] {
        for scale in [1.0, 2.0] {
            let mut harness = Harness::new();
            harness.call("ui.resize", json!({"width": w, "height": h}));
            harness.call("ui.prefs", json!({"ui_scale": scale}));
            harness.app.run("sketch.create", json!({"plane": "XY"})).unwrap();
            harness.settle();
            for mode in [LayoutMode::Floating, LayoutMode::Docked] {
                harness.app.ui.layout = mode;
                harness.settle();
            }
        }
    }
}

/// Both shell layouts share one content-hugging tree and one saved width.
/// The translucency preference never creates a second browser renderer.
#[test]
fn one_browser_keeps_its_resized_width_across_layouts_and_overlay_preferences() {
    let mut h = Harness::new();
    h.call("ui.resize", json!({"width":1400,"height":900}));
    for mode in [LayoutMode::Floating, LayoutMode::Docked] {
        for overlay in [false, true] {
            h.app.ui.layout = mode;
            h.app.preferences.browser_float = overlay;
            h.app.preferences.browser_width = 310.0;
            h.settle();
            assert!(rect_of("browser:card").is_some());
            assert_eq!(crate::scenario::handles("browser:header").len(), 1, "only one browser header");
            assert_eq!(drawn_rows().iter().filter(|(name, ..)| name == "Untitled").count(), 1, "only one tree");
            assert!((rect_of("browser:card").unwrap().width() - 310.0).abs() < 1.0);
            // Floating, a browser column pushes the scene to its right; a floating overlay browser,
            // and the docked layout (the column overlays a full-width viewport), leave it at 0.
            let expected_left = if mode == LayoutMode::Floating && !overlay { rect_of("browser:card").unwrap().right() } else { 0.0 };
            assert_eq!(h.app.viewport.rect.unwrap().left(), expected_left);
            let rail = handle_at("left-browser-rail").unwrap();
            h.call("ui.drag", json!({"x0":rail.x,"y0":rail.y,"x1":rail.x+60.0,"y1":rail.y,"steps":6}));
            h.settle();
            let width = h.app.preferences.browser_width;
            assert!(width > 340.0 && width <= MAX_BROWSER_WIDTH, "{mode:?}, overlay={overlay}: {width}");
            h.app.ui.layout = LayoutMode::Docked;
            h.app.preferences.browser_float = false;
            h.settle();
            assert!((rect_of("browser:card").unwrap().width() - width).abs() < 1.0, "compact keeps the saved width");
        }
    }
}
#[test]
fn browser_title_uses_a_single_semibold_run() {
    let ctx = crate::fonts::test_context();
    let mut app = SolveApp::new(Default::default(), Default::default());
    let output = crate::test_support::frame(&ctx, egui::RawInput::default(), |ui| {
        ui.set_width(250.0);
        browser_header(&mut app, ui, true);
    });
    let title = crate::i18n::tr("ui.browser_design_title");
    let titles: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(text) if text.galley.text() == title => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(titles.len(), 1);
    assert_eq!(titles[0].galley.job.sections[0].format.font_id, crate::fonts::semibold(Tokens::get().font_body));
}

#[test]
fn resizing_browser_card_and_column_persists_width_including_floating_preference() {
    for (mode, float, left_history) in [
        (LayoutMode::Floating, false, false),
        (LayoutMode::Docked, false, false),
        (LayoutMode::Docked, true, false),
        (LayoutMode::Floating, true, true),
        (LayoutMode::Docked, true, true),
    ] {
        let mut h = Harness::new();
        h.app.ui.layout = mode;
        h.app.preferences.browser_float = float;
        h.app.preferences.timeline_left = left_history;
        h.call("ui.resize", json!({"width":1400,"height":900}));
        h.settle();
        let before = h.app.viewport.rect.unwrap();
        let at = handle_at("left-browser-rail").unwrap();
        h.call("ui.drag", json!({"x0":at.x,"y0":at.y,"x1":at.x+80.0,"y1":at.y,"steps":8}));
        h.settle();
        let width = h.app.preferences.browser_width;
        assert!(width > crate::prefs::DEFAULT_BROWSER_WIDTH + 40.0, "{mode:?}, float={float}, width={width}");
        assert!((rect_of("browser:card").unwrap().width() - width).abs() < 1.0);
        // Floating, a browser column stays flush to the scene, so widening it moves the scene's
        // left edge while its right, top and bottom hold. Docked (the column overlays a full-width
        // viewport) and a floating overlay browser leave the projection intact.
        let now = h.app.viewport.rect.unwrap();
        if mode == LayoutMode::Floating && !float {
            assert_eq!(now.left(), rect_of("browser:card").unwrap().right(), "the scene stays flush to the widened column");
            assert_eq!((now.right(), now.top(), now.bottom()), (before.right(), before.top(), before.bottom()));
        } else {
            assert_eq!(now, before, "the projection is intact");
        }
        let mut restored = SolveApp::new(Default::default(), Default::default());
        restored.load_prefs(&h.app.prefs());
        assert_eq!(restored.preferences.browser_width, width);
        assert_eq!(restored.preferences.browser_float, float);
    }
}

#[test]
fn browser_width_rejects_hostile_preferences_and_tiny_windows_still_fit() {
    for width in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(browser_width(width), crate::prefs::DEFAULT_BROWSER_WIDTH);
    }
    assert_eq!(browser_width(-100.0), MIN_BROWSER_WIDTH);
    assert_eq!(browser_width(f32::MAX), MAX_BROWSER_WIDTH);
    let available = Rect::from_min_size(Pos2::ZERO, vec2(100.0, 300.0));
    for top_gap in [0.0, crate::shell::SHELL_GAP] {
        let bounds = panel_bounds(available, None, MAX_BROWSER_WIDTH, top_gap);
        assert!(available.contains_rect(bounds));
    }
}

#[test]
fn a_long_body_name_stays_inside_the_card_and_ends_in_an_ellipsis() {
    let mut h = Harness::new();
    h.call("ui.resize", json!({"width":1400,"height":900}));
    let name = "W".repeat(200);
    h.app.run("solid.box", json!({"body_name":name, "operation":"new", "length":10, "width":10, "height":10})).unwrap();
    h.settle();
    let card = rect_of("browser:card").unwrap();
    let label = rect_of(&format!("row_label:{name}")).expect("the body row is drawn");
    assert!(label.right() <= card.right(), "{label:?} spills out of {card:?}");
    assert!(label.width() > 100.0, "the name keeps the room it has: {label:?}");
}

#[test]
fn collapsed_rail_shares_header_hover_inset_and_expands_from_its_whole_card() {
    let previous = crate::theme::is_dark();
    for dark in [false, true] {
        crate::theme::set_dark(dark);
        for collapsed in [false, true] {
            let ctx = crate::fonts::test_context();
            ctx.global_style_mut(|style| style.animation_time = 0.0);
            let mut app = SolveApp::new(Default::default(), Default::default());
            app.ui.tab = "SURFACE".into();
            app.preferences.timeline_left = true;
            app.preferences.timeline_left_browser_collapsed = collapsed;
            crate::theme::TabAccent::publish(&app, &ctx);
            let bounds = Rect::from_min_size(Pos2::ZERO, vec2(600.0, 700.0));
            let draw = |app: &mut SolveApp, events| {
                crate::test_support::frame(&ctx, egui::RawInput { screen_rect: Some(bounds), events, ..Default::default() }, |ui| {
                    browser(app, ui);
                })
            };
            for _ in 0..3 {
                draw(&mut app, vec![]);
            }
            let card = rect_of("browser:card").unwrap();
            let header = rect_of("browser:header").unwrap();
            // Hover at the bottom margin, far away from the disclosure and rotated title.
            let at = if collapsed { card.center_bottom() - vec2(0.0, 2.0) } else { header.center() };
            draw(&mut app, vec![egui::Event::PointerMoved(at)]);
            let output = draw(&mut app, vec![egui::Event::PointerMoved(at)]);
            assert_eq!(output.platform_output.cursor_icon, egui::CursorIcon::PointingHand);
            let accent = crate::theme::TabAccent::current(&ctx);
            let mut hover = None;
            for shape in &output.shapes {
                crate::test_support::each_shape(&shape.shape, &mut |shape| {
                    if let egui::Shape::Rect(rect) = shape
                        && rect.fill == accent.soft
                    {
                        hover = Some(rect.clone());
                    }
                });
            }
            let hover =
                hover.unwrap_or_else(|| panic!("dark={dark}, collapsed={collapsed}, at={at:?}, accent={accent:?}, shapes={:?}", output.shapes));
            let inset = CARD_INSET + 1.0; // shared frame's inner margin and border
            assert_eq!(hover.rect, if collapsed { card.shrink(inset) } else { header });
            assert_eq!(hover.rect.left() - card.left(), inset);
            assert_eq!(card.right() - hover.rect.right(), inset);
            assert_eq!(hover.corner_radius, egui::CornerRadius::same(Tokens::get().radius_lg.saturating_sub(CARD_INSET as u8)));
            if collapsed {
                draw(&mut app, crate::test_support::pointer(at, true));
                draw(&mut app, crate::test_support::pointer(at, false));
                assert!(!app.preferences.timeline_left_browser_collapsed, "the bottom margin expands the rail");
            }
        }
    }
    crate::theme::set_dark(previous);
}

/// The filter field runs to the tree's right edge (where the rows' eye icons end), in both
/// layouts: its box reaches the field's right edge, and its × sits inside the box. It used to
/// stop about 28 pt short of the rows, with the × in the gap.
#[test]
fn the_filter_field_runs_to_the_trees_right_edge() {
    for layout in ["docked", "floating"] {
        let mut h = Harness::new();
        h.app.home.open = false;
        h.call("ui.set", json!({"layout": layout}));
        h.app.run("solid.box", json!({"length": 10, "width": 10, "height": 10, "body_name": "Block"})).unwrap();
        h.frames(3);
        let field = rect_of("browser:filter").unwrap();
        let eye = handle_at("eye:Block").unwrap();
        let identity = rect_of("browser:identity").unwrap();
        assert!((field.right() - (eye.x + 11.0)).abs() <= 1.0, "{layout}: the field ends where the eye icons do: {field:?}, eye at {eye:?}");
        assert!(field.right() >= identity.right() - 2.5, "{layout}: the field reaches the rows' right edge: {field:?} vs {identity:?}");
        let editor = h.ctx.read_response(egui::Id::new("sc_browser_filter")).expect("the filter editor").rect;
        assert!((editor.right() - field.right()).abs() <= 1.0, "{layout}: the editor's box spans the field: {editor:?} in {field:?}");
        h.step(&json!({"click":{"handle":"browser:filter"}})).unwrap();
        h.step(&json!({"text":"Bl"})).unwrap();
        h.frames(2);
        let clear = handle_at("browser:filter_clear").unwrap();
        assert!(field.contains(clear) && clear.x > field.right() - 20.0, "{layout}: the × sits inside the field's right end: {clear:?} in {field:?}");
        h.step(&json!({"click":{"handle":"browser:filter_clear"}})).unwrap();
        assert!(h.app.tree.filter.is_empty(), "{layout}: the × clears the filter");
    }
}

/// The `Filter features…` field keeps only the rows whose names contain its text (bodies,
/// sketches, construction geometry and features), case-insensitively, with their folders open and
/// counted; the design's settings, views and origin step aside; clearing it brings everything back.
#[test]
fn the_filter_field_keeps_only_the_rows_that_match() {
    let mut h = Harness::new();
    h.app.home.open = false;
    h.app.session.run_script(&polykrom_engine::sample::script()).unwrap();
    h.frames(3);
    let labels = || drawn_rows().into_iter().map(|(label, ..)| label).collect::<Vec<_>>();
    let all = labels();
    assert!(all.iter().any(|l| l == "Base") && all.iter().any(|l| l == "Boss") && all.iter().any(|l| l == "Origin"), "{all:?}");
    assert!(all.iter().any(|l| l == "Bodies (1)") && all.iter().any(|l| l == "Sketches (4)"), "folders carry their counts: {all:?}");
    assert!(rect_of("browser:filter").is_some());
    h.step(&json!({"click":{"handle":"browser:filter"}})).unwrap();
    h.step(&json!({"text":"hol"})).unwrap();
    h.frames(2);
    assert_eq!(h.app.tree.filter, "hol");
    let some = labels();
    assert_eq!(some.iter().filter(|l| *l == "Holes").count(), 2, "the Holes sketch and the Holes feature: {some:?}");
    assert!(!some.iter().any(|l| l == "Base" || l == "Boss" || l == "Bore" || l == "Plate" || l == "Origin"), "{some:?}");
    assert!(some.iter().any(|l| l == "Sketches (1)") && some.iter().any(|l| l == "Features (1)"), "{some:?}");
    assert!(!some.iter().any(|l| l.starts_with("Bodies")), "no body matches: its folder leaves too: {some:?}");
    h.step(&json!({"click":{"handle":"browser:filter_clear"}})).unwrap();
    h.frames(2);
    assert!(h.app.tree.filter.is_empty());
    assert_eq!(labels(), all, "clearing the filter brings every row back");
    // Case-insensitive, and a folder closed before opens while the filter is on.
    h.app.tree.collapsed.insert("c0/sketches".into());
    h.frames(2);
    assert!(!labels().iter().any(|l| l == "Boss"));
    h.app.tree.filter = "BOSS".into();
    h.frames(2);
    assert_eq!(labels().iter().filter(|l| *l == "Boss").count(), 2, "{:?}", labels());
}
