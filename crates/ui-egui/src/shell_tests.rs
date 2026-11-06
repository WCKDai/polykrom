use super::*;
use crate::scenario::{Harness, rect_of};
use crate::toolbar;
use serde_json::json;

#[test]
fn modes_are_transient_and_available_to_control_and_scenarios() {
    let mut h = Harness::new();
    assert_eq!(h.app.ui.layout, LayoutMode::Floating);
    let prefs = serde_json::to_value(&h.app.preferences).unwrap();
    let doc = h.app.session.doc.to_json();
    h.step(&json!({"call":"ui.set", "params":{"layout":"docked"}})).unwrap();
    assert_eq!(h.app.ui.layout, LayoutMode::Docked);
    h.step(&json!({"call":"ui.set", "params":{"layout":"invalid"},"fail":true})).unwrap();
    assert_eq!(h.app.ui.layout, LayoutMode::Docked);
    h.call("ui.set", json!({"layout":"floating"}));
    assert_eq!(h.app.ui.layout, LayoutMode::Floating);
    assert_eq!(h.app.session.doc.to_json(), doc);
    assert_eq!(serde_json::to_value(&h.app.preferences).unwrap(), prefs);
}

#[test]
fn both_modes_keep_their_geometry_and_fit_clear_of_covered_rects() {
    for mode in [LayoutMode::Docked, LayoutMode::Floating] {
        let mut h = Harness::new();
        h.call("ui.resize", json!({"width":1400,"height":900}));
        h.call("ui.set", json!({"layout":mode}));
        h.app.run("solid.box", json!({"length":40,"width":30,"height":20})).unwrap();
        h.settle();
        let viewport = h.app.viewport.rect.unwrap();
        let toolbar = rect_of("toolbar").unwrap();
        let status = rect_of("status:bar").unwrap();
        assert_eq!(status.bottom(), h.ctx.content_rect().bottom());
        assert!(viewport.bottom() <= status.top(), "the viewport ends above the status bar");
        let browser = rect_of("browser:card").unwrap();
        let strip = h.app.shell.document_bar.unwrap();
        if mode == LayoutMode::Floating {
            let ribbon = rect_of("ribbon:card").unwrap();
            // The scene is the stage: the window's full width, from the chrome (the ribbon card
            // and the prompt row) down to the status bar; the browser and the document strip float
            // over it, and the controls' content area starts right of the browser card.
            let prompt = rect_of("review:prompt_row").unwrap();
            assert_eq!(viewport.left(), 0.0);
            assert_eq!(viewport.right(), h.ctx.content_rect().right());
            assert_eq!(viewport.top(), prompt.bottom() + SHELL_GAP);
            assert_eq!(viewport.bottom(), status.top());
            assert!(viewport.contains_rect(browser) && viewport.contains_rect(strip));
            assert!(strip.left() >= browser.right() + SHELL_GAP - 0.5);
            assert!(h.app.shell.content.unwrap().left() >= browser.right());
            assert_eq!(ribbon.left(), SHELL_GAP);
            assert_eq!(ribbon.top(), toolbar::APP_BAR_HEIGHT + SHELL_GAP);
            assert_eq!(ribbon.right(), h.ctx.content_rect().right() - SHELL_GAP);
            assert!(covered(&h.app, ribbon.center()));
        } else {
            assert_eq!(toolbar.top(), toolbar::APP_BAR_HEIGHT);
            // The AI review's prompt row sits between the ribbon and the viewport (design kit).
            let prompt = rect_of("review:prompt_row").unwrap();
            assert_eq!(prompt.top(), toolbar.bottom());
            // The document tabs and the view bar sit between the prompt row and the scene.
            let strip = h.app.shell.document_bar.unwrap();
            assert_eq!(strip.top(), prompt.bottom());
            assert_eq!(viewport.top(), strip.bottom());
            assert_eq!(toolbar.height(), toolbar::RIBBON_HEIGHT);
            assert_eq!(toolbar.left(), 0.0);
            // The document bar yields its bottom row to the scene it abuts, so its covered rect is
            // a pixel shorter than the drawn strip.
            let strip_covered = egui::Rect::from_min_max(strip.min, pos2(strip.max.x, strip.max.y - 1.0));
            assert_eq!(covered_rects(&h.app), vec![h.app.shell.history.unwrap(), browser, strip_covered]);
        }
        let _ = strip;
        h.app.fit_view();
        h.settle();
        let layout = h.app.viewport.layout.as_ref().unwrap();
        for covered in covered_rects(&h.app) {
            assert!(!layout.drawable.intersect(covered).is_positive(), "{layout:?} {covered:?}");
        }
        let projection = crate::viewport::projection(&h.app, viewport);
        for point in [polykrom_engine::geom::Vec3::ZERO, polykrom_engine::geom::Vec3::new(40.0, 30.0, 20.0)] {
            assert!(layout.drawable.contains(projection.to_screen(point).unwrap()));
        }
        h.app.start("solid.extrude");
        h.settle();
        if mode == LayoutMode::Floating {
            assert!(h.app.viewport.dialog_rect.unwrap().top() >= h.app.viewport.layout.as_ref().unwrap().axis_view.bottom());
        }
    }
}

#[test]
fn cards_block_model_hover_wheel_and_drag_but_the_gap_remains_viewport() {
    let mut h = Harness::new();
    h.app.run("solid.box", json!({"length":40,"width":30,"height":20})).unwrap();
    h.settle();
    let ribbon = rect_of("ribbon:card").unwrap();
    // Blank space at the right, away from the search and command buttons.
    let at = pos2(ribbon.right() - 12.0, ribbon.bottom() - 28.0);
    let camera = h.app.cam;
    let doc = h.app.session.doc.to_json();
    h.call("ui.move", json!({"x":at.x,"y":at.y}));
    assert!(h.app.viewport.hover.is_none());
    assert!(h.app.viewport.mouse.is_none());
    h.call("ui.scroll", json!({"x":at.x,"y":at.y,"delta":120}));
    h.call("ui.drag", json!({"x0":at.x,"y0":at.y,"x1":at.x-30.0,"y1":at.y+15.0,"button":"middle","steps":6}));
    assert_eq!(h.app.cam, camera);
    assert!(h.app.viewport.boxsel.is_none());
    assert_eq!(h.app.session.doc.to_json(), doc);
    // The scene begins just right of the browser column.
    let browser = rect_of("browser:card").unwrap();
    let gap = pos2(browser.right() + 2.0, browser.center().y);
    h.call("ui.move", json!({"x":gap.x,"y":gap.y}));
    assert_eq!(h.app.viewport.mouse, Some(gap));
}

#[test]
fn floating_sketch_palette_and_compass_share_measured_clearance() {
    for style in crate::prefs::NavigatorStyle::ALL {
        // The short window keeps 43 points more than it did for the AI review's prompt row under
        // the ribbon, so it is as short as before relative to the controls.
        // The heights carry the design kit's shell (the document tabs and the view bar, the
        // taller history strip and the status bar) on top of the room the palette needs.
        for (width, height, scale) in [(1400.0, 1080.0, 1.0), (700.0, 960.0, 2.0)] {
            let mut h = Harness::new();
            h.app.preferences.navigator_style = style;
            h.call("ui.resize", json!({"width":width,"height":height}));
            h.call("ui.prefs", json!({"ui_scale":scale}));
            h.app.run("sketch.create", json!({"plane":"XY"})).unwrap();
            h.app.run("sketch.line", json!({"points":[[0,0],[20,0]]})).unwrap();
            h.settle();
            h.app.fit_view();
            h.settle();
            let layout = h.app.viewport.layout.as_ref().unwrap();
            let palette = h.app.viewport.palette_rect.unwrap();
            assert!(palette.top() >= layout.axis_view.bottom(), "{style:?} {palette:?} {layout:?}");
            assert!(layout.viewport.expand(1.0).contains_rect(palette), "{palette:?} {layout:?}");
            assert!(palette.contains(crate::scenario::handle_at("sketch_palette:look_at").unwrap()));
            let proj = crate::viewport::projection(&h.app, layout.viewport);
            for point in [polykrom_engine::geom::Vec3::ZERO, polykrom_engine::geom::Vec3::new(20.0, 0.0, 0.0)] {
                assert!(layout.drawable.contains(proj.to_screen(point).unwrap()), "{layout:?}");
            }
            if width > 1000.0 {
                let content = layout.content;
                h.app.start("solid.extrude");
                h.settle();
                let dialog = h.app.viewport.dialog_rect.unwrap();
                let expected_top =
                    (content.top() + crate::viewport::AXIS_VIEW_CLEARANCE).min((content.bottom() - PANEL_MIN_HEIGHT).max(content.top()));
                assert!((dialog.left() - content.left()).abs() < 1.0);
                assert!((dialog.top() - expected_top).abs() < 1.0, "left dialogs retain their existing top: {dialog:?}");
            }
        }
    }
}

#[test]
fn short_floating_palette_stays_in_viewport() {
    for style in crate::prefs::NavigatorStyle::ALL {
        for height in [475, 385, 300] {
            let mut h = Harness::new();
            h.app.preferences.navigator_style = style;
            h.call("ui.resize", json!({"width":700,"height":height}));
            h.call("ui.prefs", json!({"ui_scale":2.0}));
            h.app.run("sketch.create", json!({"plane":"XY"})).unwrap();
            // Main's open/untied-end feedback also consumes footer space.
            for points in [[[0, 0], [10, 0]], [[10, 0], [10, 10]], [[10, 10], [0, 10]], [[0, 10], [0, 0]], [[20, 0], [30, 5]]] {
                h.app.run("sketch.line", json!({"points":points,"infer":false,"connect":false})).unwrap();
            }
            // Include the extra Feature Options section in the tiny-height case.
            if height == 300 {
                h.app.start("sketch.rectangle.two_point");
            }
            h.settle();
            let viewport = h.app.viewport.rect.unwrap();
            let palette = h.app.viewport.palette_rect.unwrap();
            assert!(
                viewport.contains_rect(palette),
                "{style:?}, height={height}: {palette:?} extends beyond {viewport:?}, right={:.9}, bottom={:.9}",
                palette.right(),
                palette.bottom()
            );
            let compass = h.app.viewport.layout.as_ref().unwrap().axis_view;
            if compass.is_positive() {
                assert!(palette.top() < compass.bottom(), "short panels reclaim compass space");
            }
            let action = crate::scenario::handle_at("sketch_palette:look_at").unwrap();
            if !palette.contains(action) {
                // Once reclaiming is insufficient, the whole palette scrolls to its footer.
                h.call("ui.scroll", json!({"x":palette.center().x,"y":palette.center().y,"delta":-1200}));
                h.frames(20);
            }
            let action = crate::scenario::handle_at("sketch_palette:look_at").unwrap();
            assert!(palette.contains(action), "footer must be reachable: {style:?}, height={height}, {action:?} in {palette:?}");
            let finish = crate::scenario::handle_at("sketch_palette:finish").unwrap();
            h.call("ui.click", json!({"x":finish.x,"y":finish.y}));
            assert!(h.app.session.active_sketch.is_none(), "Finish Sketch must work after scrolling");
        }
    }
}

#[test]
fn selection_drag_released_over_a_card_leaves_no_rubber_band() {
    let mut h = Harness::new();
    h.call("ui.resize", json!({"width":1400,"height":900}));
    h.settle();
    let ribbon = rect_of("ribbon:card").unwrap();
    let viewport = h.app.viewport.rect.unwrap();
    h.call(
        "ui.drag",
        json!({"x0":viewport.right()-20.0,"y0":viewport.bottom()-100.0,
        "x1":ribbon.right()-12.0,"y1":ribbon.bottom()-28.0,"steps":12}),
    );
    assert!(h.app.viewport.boxsel.is_none());
    assert!(h.app.session.selection.is_empty());
}

#[test]
fn floating_browser_clears_cards_shields_input_and_can_dock_again() {
    for mode in [LayoutMode::Floating, LayoutMode::Docked] {
        for timeline_left in [false, true] {
            let mut h = Harness::new();
            h.call("ui.set", json!({"layout":mode}));
            h.app.preferences.timeline_left = timeline_left;
            h.app.preferences.browser_float = true;
            h.app.run("solid.box", json!({"length":40,"width":30,"height":20})).unwrap();
            h.settle();
            let browser = rect_of("browser:float").unwrap();
            let toolbar = rect_of("toolbar").unwrap();
            assert!(browser.top() >= toolbar.bottom(), "{mode:?}: {browser:?} must clear {toolbar:?}");
            assert!(crate::scenario::handle_at("row:Untitled").is_some());
            assert!(crate::scenario::handle_at("left-browser-rail").is_some());
            assert!(covered(&h.app, browser.center()));
            let viewport = h.app.viewport.rect.unwrap();
            // A floating overlay browser leaves the scene its full width; only a docked vertical
            // history (an edge panel) takes a column that pushes the scene right. Floating, the
            // history is a card over the full-bleed scene too.
            if timeline_left && mode == LayoutMode::Docked {
                assert!(viewport.left() > 0.0);
            } else {
                assert_eq!(viewport.left(), 0.0);
            }
            if mode == LayoutMode::Floating {
                // The scene starts right under the top chrome (the prompt row and its gap); the
                // document strip floats inside it.
                assert_eq!(viewport.top(), rect_of("review:prompt_row").unwrap().bottom() + SHELL_GAP);
                assert!(h.app.shell.document_bar.is_some_and(|s| viewport.contains_rect(s)));
                assert!(!browser.intersect(rect_of("ribbon:card").unwrap()).is_positive());
            }
            h.app.fit_view();
            h.settle();
            assert!(!h.app.viewport.layout.as_ref().unwrap().drawable.intersect(browser).is_positive());
            let camera = h.app.cam;
            let doc = h.app.session.doc.to_json();
            let at = browser.center();
            h.call("ui.move", json!({"x":at.x,"y":at.y}));
            assert!(h.app.viewport.hover.is_none());
            assert!(h.app.viewport.mouse.is_none());
            h.call("ui.scroll", json!({"x":at.x,"y":at.y,"delta":120}));
            h.call("ui.drag", json!({"x0":at.x,"y0":at.y,"x1":at.x+20.0,"y1":at.y+10.0,"button":"middle","steps":6}));
            assert_eq!(h.app.cam, camera);
            assert_eq!(h.app.session.doc.to_json(), doc);
            // The browser remains interactive above the shielded viewport.
            h.step(&json!({"click":{"chevron":"Origin"}})).unwrap();
            assert!(crate::browser::drawn_rows().iter().any(|(name, _, _, open)| name == "Origin" && *open == Some(true)));
            h.app.preferences.browser_float = false;
            h.settle();
            assert!(h.app.shell.browser.is_some());
            assert!(rect_of("browser:float").is_none());
            assert!(rect_of("browser:card").is_some());
            // The saved width follows both the floating card and the docked column.
            let rail = crate::scenario::handle_at("left-browser-rail").unwrap();
            let width = h.app.preferences.browser_width;
            h.call("ui.drag", json!({"x0":rail.x,"y0":rail.y,"x1":rail.x+60.0,"y1":rail.y,"steps":6}));
            assert!(h.app.preferences.browser_width > width + 30.0);
        }
    }
}

#[test]
fn browser_and_history_cards_block_input_while_their_gaps_and_space_below_browser_do_not() {
    let mut h = Harness::new();
    h.call("ui.resize", json!({"width":1400,"height":900}));
    h.app.run("solid.box", json!({"length":40,"width":30,"height":20})).unwrap();
    h.settle();
    let browser = rect_of("browser:card").unwrap();
    let history = rect_of("history:card").unwrap();
    let viewport = h.app.viewport.rect.unwrap();
    // The history card floats over the bottom of the scene, inset by the shell gap; the browser
    // card floats over its left, and the controls' content area starts right of the card.
    assert_eq!(history.left(), SHELL_GAP);
    assert_eq!(history.right(), viewport.right() - SHELL_GAP);
    assert_eq!(history.bottom(), viewport.bottom() - SHELL_GAP);
    assert_eq!(viewport.left(), 0.0);
    assert!(h.app.shell.content.unwrap().left() >= browser.right());
    let layout = h.app.viewport.layout.as_ref().unwrap();
    assert!(layout.navigation.bottom() < history.top());
    assert!(layout.axis_view.top() >= rect_of("ribbon:card").unwrap().bottom());
    for r in [browser, history] {
        assert!(covered(&h.app, r.center()));
        assert!(!layout.drawable.intersect(r).is_positive());
    }
    for at in [pos2(browser.left() + 2.0, browser.top() + 2.0), pos2(history.right() - 30.0, history.center().y)] {
        let camera = h.app.cam;
        h.call("ui.move", json!({"x":at.x,"y":at.y}));
        assert!(h.app.viewport.mouse.is_none());
        assert!(h.app.viewport.hover.is_none());
        h.call("ui.scroll", json!({"x":at.x,"y":at.y,"delta":120}));
        h.call("ui.drag", json!({"x0":at.x,"y0":at.y,"x1":at.x+10.0,"y1":at.y+2.0,"button":"middle","steps":6}));
        assert_eq!(h.app.cam, camera);
    }
    // The open scene, right of the browser column and between the strip and the history, takes
    // the pointer.
    for at in [pos2(browser.right() + 40.0, viewport.center().y), pos2(viewport.center().x, history.top() - 20.0)] {
        assert!(!covered(&h.app, at));
        h.call("ui.move", json!({"x":at.x,"y":at.y}));
        assert_eq!(h.app.viewport.mouse, Some(at));
    }
    h.app.ui.show_browser = false;
    h.app.ui.show_timeline = false;
    h.settle();
    assert!(h.app.shell.browser.is_none() && h.app.shell.history.is_none());
    assert!(!covered(&h.app, browser.center()));
    assert!(!covered(&h.app, history.center()));
}

#[test]
fn left_history_and_browser_share_the_left_edge_without_moving_right_controls() {
    for mode in [LayoutMode::Floating, LayoutMode::Docked] {
        let mut h = Harness::new();
        h.app.ui.layout = mode;
        h.app.preferences.timeline_left = true;
        h.app.run("solid.box", json!({"length":10,"width":10,"height":10,"name":"First"})).unwrap();
        h.settle();
        assert!(rect_of("history:card").is_none());
        let history = rect_of("history:vertical").unwrap();
        let browser = rect_of("browser:card").unwrap();
        assert!((browser.left() - history.right() - Container::for_layout(mode, Surface::Panel).gap()).abs() < 1.0);
        assert!(crate::scenario::handle_at("timeline:First").is_some());
        let viewport = h.app.viewport.rect.unwrap();
        let content = content_rect(&h.app, viewport);
        // The vertical history and the browser share the left edge. Floating, both are cards over
        // the full-bleed scene; docked, the viewport spans from the history column's right with
        // the browser overlaying it. The content area starts past both.
        assert_eq!(viewport.left(), if mode == LayoutMode::Floating { 0.0 } else { history.right() });
        assert!(content.left() >= history.right(), "the content area starts right of the vertical history");
        assert_eq!(content.bottom(), viewport.bottom());
        if mode == LayoutMode::Floating {
            assert!(covered(&h.app, history.center()));
            assert!(covered(&h.app, browser.center()));
            let gap_x = (history.right() + browser.left()) / 2.0;
            assert!(covered(&h.app, pos2(gap_x, history.center().y)), "the seam pill shields viewport input");
            assert!(!covered(&h.app, pos2(gap_x, history.top() + 4.0)), "the rest of the shell gap remains available");
        }
    }
}

#[test]
fn assistant_floats_without_changing_projection_and_shields_the_model() {
    for dark in [false, true] {
        for side in ["left", "right"] {
            let mut h = crate::scenario::Harness::new();
            h.app.home.open = false;
            h.call("ui.resize", serde_json::json!({"width":1400,"height":1000}));
            h.call("ui.set", serde_json::json!({"dark":dark,"layout":"floating"}));
            h.frames(4);
            let projection = h.app.viewport.rect.expect("viewport");
            let original_controls = h.app.viewport.layout.clone().expect("layout");
            h.call("ui.agent_chat", serde_json::json!({"open":true,"picker":false,"side":side,"width":440}));
            h.frames(6);
            let card = crate::scenario::rect_of("agent_chat:panel").expect("floating Assistant");
            assert_eq!(h.app.viewport.rect, Some(projection), "opening the card preserves projection");
            assert!(card.left() > projection.left() && card.right() < projection.right());
            assert!(card.bottom() < projection.bottom());
            let content = h.app.shell.content.expect("content");
            // The card reaches the top of the content area (the document bar sits beside it).
            let top = h.app.shell.document_bar.map_or(content.top(), |strip| strip.top());
            assert!((card.top() - top).abs() < 1.0, "full-height top: {card:?} {content:?}");
            assert!((card.bottom() - content.bottom()).abs() < 1.0, "full-height bottom: {card:?} {content:?}");
            assert!(covered(&h.app, card.center()), "chat shields viewport input");
            if side == "right" {
                let layout = h.app.viewport.layout.as_ref().expect("layout");
                assert!(!card.intersect(layout.axis_view).is_positive(), "card clears compass");
                assert!(!card.intersect(layout.navigation).is_positive(), "card clears navigation");
                assert!(layout.axis_view.right() < original_controls.axis_view.right(), "compass moves left");
                assert!(layout.navigation.center().x < original_controls.navigation.center().x, "navigation moves left");
            } else {
                // The left Assistant floats over the scene beside the full-height browser column
                // (view 01), clear of it, without moving the model's projection.
                let browser = h.app.shell.browser.expect("browser card");
                assert!(card.left() >= browser.right(), "left Assistant floats beside the browser column: {card:?} {browser:?}");
            }
            let camera = h.app.cam;
            let at = card.center();
            h.call("ui.move", json!({"x":at.x,"y":at.y}));
            h.call("ui.scroll", json!({"x":at.x,"y":at.y,"delta":120}));
            h.call("ui.drag", json!({"x0":at.x,"y0":at.y,"x1":at.x-30.0,"y1":at.y+15.0,"button":"middle","steps":6}));
            assert_eq!(h.app.cam, camera, "chat consumes wheel and orbit input");
            assert!(h.app.viewport.boxsel.is_none());
            h.app.start("solid.extrude");
            h.frames(5);
            let feature = h.app.viewport.dialog_rect.expect("feature panel");
            let beside = crate::scenario::rect_of("agent_chat:panel").expect("Assistant beside feature");
            assert!(!beside.intersect(feature).is_positive(), "feature and Assistant coexist: {beside:?} {feature:?}");
            assert!(
                (beside.min - card.min).length() < 1.0 && (beside.max - card.max).length() < 1.0,
                "opening a tool must not move Assistant: {card:?} -> {beside:?}"
            );
            if side == "right" {
                assert!(feature.right() + SHELL_GAP <= beside.left() + 1.0, "tool stays inside Assistant's rail");
            }
            h.app.dialog = None;
            h.app.tool = None;
            h.frames(3);
            h.call("ui.agent_chat", serde_json::json!({"minimized":true}));
            h.frames(4);
            let folded = crate::scenario::rect_of("agent_chat:panel").expect("floating folded card");
            assert!(folded.width() < 60.0 && folded.height() < 120.0);
            assert!(covered(&h.app, folded.center()));
            assert_eq!(h.app.viewport.rect, Some(projection));
            let folded_controls = h.app.viewport.layout.as_ref().expect("layout");
            assert_eq!(folded_controls.axis_view, original_controls.axis_view);
            if side == "right" {
                assert_eq!(folded_controls.navigation, original_controls.navigation);
            } else {
                // The minimized Assistant floats over the scene without reserving a column, so it
                // leaves the navigation where it was (the model's controls do not move for it).
                assert_eq!(folded_controls.navigation, original_controls.navigation);
            }
            h.call("ui.agent_chat", serde_json::json!({"open":false}));
            h.frames(3);
            // Probed beside the browser column, which keeps the card's inner edge when it closes.
            let probe = if side == "right" { pos2(card.left() + 10.0, card.center().y) } else { pos2(card.right() - 10.0, card.center().y) };
            assert!(!covered(&h.app, probe), "closed chat leaves no stale obstruction");
        }
    }
}

#[test]
fn assistant_left_dock_precedes_history_and_drag_hints_match_the_result() {
    for layout in ["floating", "docked"] {
        for (names, stacked) in [(false, false), (true, false), (true, true)] {
            let mut h = Harness::new();
            h.app.home.open = false;
            h.app.preferences.timeline_left = true;
            h.app.preferences.timeline_under_browser = stacked;
            h.app.preferences.timeline_left_width = crate::ui_kit::history_width(names);
            h.call("ui.resize", json!({"width":1600,"height":1000}));
            h.call("ui.set", json!({"layout":layout}));
            h.call("ui.agent_chat", json!({"open":true,"side":"right","width":440,"picker":false}));
            h.frames(6);
            let projection = h.app.viewport.rect;
            for left in [true, false] {
                let grip = crate::scenario::handle_at("agent_chat:grip").expect("drag grip");
                let target = pos2(if left { 100.0 } else { 1500.0 }, grip.y);
                h.app.synthetic.push(egui::Event::PointerMoved(grip));
                h.frame();
                h.app.synthetic.push(egui::Event::PointerButton {
                    pos: grip,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                });
                h.frame();
                h.app.synthetic.push(egui::Event::PointerMoved(target));
                h.frames(2);
                let hint = rect_of("agent_chat:dock_hint").expect("visible dock target");
                h.app.synthetic.push(egui::Event::PointerButton {
                    pos: target,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                });
                h.frames(6);
                let card = h.app.agent_chat.panel_rect.expect("docked Assistant");
                assert_eq!(h.app.preferences.agent_chat_side, if left { "left" } else { "right" });
                assert!((hint.min - card.min).length() < 1.0 && (hint.max - card.max).length() < 1.0, "{layout} preview {hint:?} matches {card:?}");
                if layout == "floating" {
                    assert_eq!(h.app.viewport.rect, projection, "docking keeps model projection");
                }
                if left {
                    let history = h.app.shell.history.expect("vertical history");
                    let browser = h.app.shell.browser.expect("browser");
                    // Compact (docked) the Assistant reserves the outer left edge, before the
                    // vertical history and the browser column; floating it docks as a card beside
                    // them, over the scene, clear of the history column.
                    if layout == "docked" {
                        assert!(card.right() <= history.left(), "Assistant precedes history: {card:?} {history:?}");
                    } else {
                        assert!(card.left() >= history.right(), "Assistant docks beside history: {card:?} {history:?}");
                    }
                    if stacked {
                        assert!((history.left() - browser.left()).abs() < 1.0);
                        assert!((history.width() - browser.width()).abs() < 1.0);
                        assert!(browser.bottom() <= history.top(), "History stays below Browser");
                    } else {
                        assert!(history.right() <= browser.left(), "history precedes browser");
                    }
                }
            }
            h.call("ui.agent_chat", json!({"side":"left","minimized":true}));
            h.frames(6);
            let strip = h.app.agent_chat.drop_rect.expect("minimized Assistant");
            let history_left = h.app.shell.history.expect("history").left();
            // Compact (docked) the minimized Assistant keeps the outer edge; floating it sits as a
            // strip beside the history column, clear of it.
            if layout == "docked" {
                assert!(strip.right() <= history_left);
            } else {
                assert!(strip.left() >= h.app.shell.history.expect("history").right());
            }
            h.call("ui.agent_chat", json!({"open":false}));
            h.frames(6);
            assert!(h.app.shell.history.expect("history keeps the outer edge").left() <= history_left + 1.0);
        }
    }
}

#[test]
fn assistant_right_edge_survives_palette_tool_resize_and_minimize() {
    for layout in ["floating", "docked"] {
        let mut h = Harness::new();
        h.app.home.open = false;
        h.call("ui.resize", json!({"width":1800,"height":1000}));
        h.call("ui.set", json!({"layout":layout}));
        h.call("ui.agent_chat", json!({"open":true,"side":"right","width":380,"picker":false}));
        h.frames(5);
        let right = h.app.agent_chat.panel_rect.unwrap().right();
        h.app.run("sketch.create", json!({"plane":"XY"})).unwrap();
        h.frames(6);
        for width in [380.0, 480.0] {
            h.call("ui.agent_chat", json!({"width":width}));
            h.frames(6);
            let chat = h.app.agent_chat.panel_rect.unwrap();
            let palette = h.app.viewport.palette_rect.unwrap();
            assert!((chat.right() - right).abs() < 1.0, "{layout}: Assistant stays outside the palette");
            assert!(
                palette.right() + Container::for_layout(h.app.ui.layout, Surface::Panel).gap() <= chat.left() + 1.0,
                "{layout}: {palette:?}, {chat:?}"
            );
        }
        h.call("ui.agent_chat", json!({"minimized":true}));
        h.frames(5);
        let strip = h.app.agent_chat.drop_rect.unwrap();
        let palette = h.app.viewport.palette_rect.unwrap();
        assert!((strip.right() - right).abs() < 1.0);
        assert!(palette.right() <= strip.left(), "palette clears the minimized Assistant too");
        h.call("ui.agent_chat", json!({"open":false}));
        h.frames(5);
        assert!(h.app.viewport.palette_rect.unwrap().right() > palette.right(), "closing Assistant reclaims its rail");
    }
}

#[test]
fn tool_sheets_drag_over_browser_and_history_but_clear_protected_panels() {
    for side in ["left", "right"] {
        let mut h = Harness::new();
        h.app.home.open = false;
        h.call("ui.resize", json!({"width":1800,"height":1000}));
        h.call("ui.set", json!({"layout":"floating"}));
        h.call("ui.agent_chat", json!({"open":true,"side":side,"width":380,"picker":false}));
        h.frames(5);
        h.app.start("sketch.create");
        h.frames(5);
        let chat = h.app.agent_chat.panel_rect.unwrap();
        let browser = h.app.shell.browser.unwrap();
        let history = h.app.shell.history.unwrap();
        let ribbon = h.app.shell.cards[0];
        for target in [browser.center(), history.center(), ribbon.center(), egui::pos2(-1000.0, 400.0), egui::pos2(3000.0, 400.0)] {
            let title = crate::scenario::handle_at("dialog:title").unwrap();
            h.call("ui.drag", json!({"x0":title.x,"y0":title.y,"x1":target.x,"y1":target.y,"steps":10}));
            h.frames(4);
            let sheet = h.app.viewport.dialog_rect.unwrap();
            assert!(!sheet.intersect(chat).is_positive(), "{side}: {sheet:?} overlaps Assistant {chat:?}");
            assert!(!sheet.intersect(ribbon).is_positive(), "sheet clears ribbon");
            if target == browser.center() {
                // The browser is a card over the scene, like History: a tool sheet may cover it,
                // unless the Assistant stands between them (a left Assistant floats beside the
                // browser card, and the sheet stays clear of the Assistant).
                if side == "left" {
                    assert!(!sheet.intersect(browser).is_positive(), "the sheet stops at the Assistant, short of the browser card");
                } else {
                    assert!(sheet.intersect(browser).is_positive(), "sheet can cover the browser card");
                }
            } else if target == history.center() {
                assert!(sheet.intersect(history).is_positive(), "sheet can cover History");
            }
        }
        h.step(&json!({"widget":"Cancel"})).unwrap();
        assert!(h.app.dialog.is_none(), "dragged sheet remains interactive");
    }
}

#[test]
fn compact_layout_is_saved_and_survives_cancel_apply_defaults_and_bad_input() {
    let mut h = Harness::new();
    h.app.run("solid.box", json!({"length":10,"width":10,"height":10})).unwrap();
    h.app.start("solid.extrude");
    h.settle();
    let design = h.app.session.doc.to_json();
    let undo = h.app.session.undo.len();
    let camera = h.app.cam;
    h.call("ui.prefs", json!({"open":"Interface"}));
    crate::prefs::reveal_preference(&mut h, "dialog:Workspace:Compact");
    h.step(&json!({"click":{"handle":"dialog:Workspace:Compact"}})).unwrap();
    h.call("ui.prefs", json!({"button":"ok"}));
    h.settle();
    assert!(h.app.dialog.is_some(), "changing layout keeps the active tool");
    assert_eq!(h.app.preferences.layout, LayoutMode::Docked);
    assert_eq!(h.app.ui.layout, LayoutMode::Docked);
    assert_eq!(h.app.cam, camera);
    h.call("engine.execute", json!({"command":"view.layout", "params":{"mode":"compact"}}));
    assert_eq!(h.app.cam, camera, "programmatic layout changes never auto-fit");
    assert_eq!(h.app.session.doc.to_json(), design);
    assert_eq!(h.app.session.undo.len(), undo);
    let mut restored = crate::SolveApp::new(Default::default(), Default::default());
    restored.load_prefs(&h.app.prefs());
    assert_eq!(restored.ui.layout, LayoutMode::Docked);
    assert_eq!(restored.preferences.layout, LayoutMode::Docked);
    h.call("ui.prefs", json!({"open":"Interface","layout":"floating"}));
    h.call("ui.prefs", json!({"button":"cancel"}));
    assert_eq!(h.app.ui.layout, LayoutMode::Docked);
    h.call("ui.prefs", json!({"open":"Interface","layout":"floating"}));
    h.call("ui.prefs", json!({"button":"apply"}));
    h.call("ui.prefs", json!({"layout":"compact"}));
    h.call("ui.prefs", json!({"button":"cancel"}));
    assert_eq!(h.app.ui.layout, LayoutMode::Floating);
    h.call("ui.prefs", json!({"open":"Interface","layout":"compact"}));
    h.call("ui.prefs", json!({"button":"defaults"}));
    assert_eq!(h.app.ui.layout, LayoutMode::Floating);
    assert_eq!(h.app.preferences.layout, LayoutMode::Floating);
    h.call("ui.prefs", json!({"button":"ok"}));
    assert!(crate::prefs::set(&mut h.app, &json!({"layout":"invalid"})).is_err());
    assert_eq!(h.app.ui.layout, LayoutMode::Floating);
    restored.load_prefs(r#"{"preferences":{"layout":"invalid","default_units":"in"}}"#);
    assert_eq!(restored.ui.layout, LayoutMode::Floating);
    assert_eq!(restored.preferences.default_units, "in");
    assert_eq!(crate::prefs::lenient(&json!({})).layout, LayoutMode::Floating);
}

#[test]
fn compact_chrome_is_flush_and_browser_fills_its_column_in_both_themes() {
    for dark in [false, true] {
        let mut h = Harness::new();
        h.call("ui.resize", json!({"width":1400,"height":900}));
        h.call("ui.prefs", json!({"layout":"compact"}));
        h.call("ui.set", json!({"dark":dark}));
        h.settle();
        let bar = rect_of("toolbar").unwrap();
        let browser = rect_of("browser:card").unwrap();
        assert_eq!(h.app.shell.cards.len(), 1, "one joined top bar");
        assert_eq!(bar.left(), 0.0);
        assert_eq!(browser.left(), 0.0);
        // The AI review's prompt row joins the ribbon; the Browser joins it, square and flush.
        let prompt = rect_of("review:prompt_row").unwrap();
        assert_eq!(prompt.top(), bar.bottom(), "square, flush prompt row/ribbon join");
        assert_eq!(prompt.left(), 0.0);
        assert_eq!(browser.top(), prompt.bottom(), "square, flush Browser/prompt row join");
        // The Design Browser is a full-height column down to the history strip.
        assert_eq!(browser.bottom(), h.app.shell.history.unwrap().top(), "the browser reaches the history strip");
        let compact = crate::ui_kit::Container::new(&h.ctx, crate::ui_kit::Surface::Panel);
        assert_eq!(compact.gap(), 0.0);
        let frame = compact.frame(&h.ctx, crate::theme::Tokens::get().panel, crate::frame::Side::Left);
        assert_eq!(frame.shadow, egui::Shadow::NONE);
        assert_eq!(frame.corner_radius, egui::CornerRadius::ZERO);
        h.app.start("solid.extrude");
        h.settle();
        let dialog = h.app.viewport.dialog_rect.unwrap();
        assert!((dialog.right() - h.app.viewport.rect.unwrap().right()).abs() < 1.0, "{dialog:?}");
        let before = dialog;
        h.step(&json!({"drag":[{"handle":"dialog:title"},[700,300]],"steps":6})).unwrap();
        h.settle();
        let moved = h.app.viewport.dialog_rect.unwrap();
        assert!(moved.right() < before.right() - 100.0, "compact tool sheets detach from the right edge");
        let feature = crate::ui_kit::Container::new(&h.ctx, crate::ui_kit::Surface::Feature);
        assert_eq!(feature.frame(&h.ctx, crate::theme::Tokens::get().dialog_body, crate::frame::Side::Right).shadow, egui::Shadow::NONE);
        h.app.run("view.layout", json!({"mode":"floating"})).unwrap();
        h.settle();
        assert_eq!(rect_of("browser:card").unwrap().left(), SHELL_GAP);
        assert!(h.app.dialog.is_some());
    }
}

#[test]
fn two_collapsed_compact_headers_leave_live_viewport_below_their_column() {
    for dark in [false, true] {
        let mut h = Harness::new();
        h.call("ui.resize", json!({"width":1400,"height":900}));
        h.call("ui.prefs", json!({"layout":"compact"}));
        h.call("ui.set", json!({"dark":dark}));
        h.app.run("view.history_layout", json!({"dock":"under_browser"})).unwrap();
        h.app.preferences.timeline_left_browser_collapsed = true;
        h.app.preferences.timeline_collapsed = true;
        h.frames(12);
        let browser = h.app.shell.browser.unwrap();
        let history = h.app.shell.history.unwrap();
        let below = egui::pos2(browser.center().x, history.bottom() + 40.0);
        assert!(h.app.viewport.rect.unwrap().contains(below));
        assert!(!covered(&h.app, below));
        assert!(covered(&h.app, browser.center()));
        assert!(covered(&h.app, history.center()));
        h.call("ui.move", json!({"x":below.x,"y":below.y}));
        assert_eq!(h.app.viewport.mouse, Some(below), "the vacated column remains live viewport");
    }
}

#[test]
fn ribbon_labels_are_saved_independently_and_icon_only_commands_remain_reachable() {
    for mode in [LayoutMode::Floating, LayoutMode::Docked] {
        let mut h = Harness::new();
        h.call("ui.resize", json!({"width":1400,"height":900}));
        h.call("ui.prefs", json!({"layout":mode}));
        h.settle();
        let labelled = rect_of("toolbar").unwrap();
        h.call("ui.prefs", json!({"ribbon_labels":false}));
        h.settle();
        let icons = rect_of("toolbar").unwrap();
        assert!(icons.height() < labelled.height());
        assert_eq!(h.app.ui.layout, mode);
        let command = rect_of("button:sketch.create").unwrap();
        assert!(icons.contains_rect(command));
        h.step(&json!({"click":{"handle":"button:sketch.create"}})).unwrap();
        assert!(h.app.dialog.is_some(), "the icon still starts Create Sketch");
        let mut restored = crate::SolveApp::new(Default::default(), Default::default());
        restored.load_prefs(&h.app.prefs());
        assert!(!restored.preferences.ribbon_labels);
        assert_eq!(restored.ui.layout, mode);
        h.call("ui.prefs", json!({"open":"Interface","ribbon_labels":true}));
        h.call("ui.prefs", json!({"button":"cancel"}));
        assert!(!h.app.preferences.ribbon_labels);
    }
    assert!(crate::prefs::lenient(&json!({})).ribbon_labels);
}

/// The window sizes the owner's direction is checked at: a laptop's, the default and the kit's.
const STAGE_WINDOWS: [(f32, f32); 3] = [(1366.0, 768.0), (1600.0, 1000.0), (1760.0, 1100.0)];

/// The canvas is the stage (the owner's direction, 2026-11-06): in the default layout the scene
/// spans the window's full width and all the height between the top chrome (the title bar, the
/// workspace tabs and the ribbon, the AI prompt row) and the status bar, at every size. The
/// Design Browser, the document strip and the history are cards inside it, none takes a band or
/// a column out of it, and the chrome above it is shorter than the kit's shell was (a 36 px title
/// bar with a smaller logo under a 246 px chrome).
#[test]
fn the_canvas_spans_the_window_under_a_shorter_chrome_at_every_size() {
    for (w, hgt) in STAGE_WINDOWS {
        let mut h = Harness::new();
        h.app.home.open = false;
        h.call("ui.resize", json!({"width": w, "height": hgt}));
        h.app.run("solid.box", json!({"length": 40, "width": 30, "height": 20})).unwrap();
        h.frames(3);
        assert_eq!(h.app.ui.layout, LayoutMode::Floating, "the default layout");
        let scene = h.app.viewport.rect.unwrap();
        let prompt = rect_of("review:prompt_row").unwrap();
        let ribbon = rect_of("ribbon:card").unwrap();
        let status = rect_of("status:bar").unwrap();
        assert_eq!((scene.left(), scene.right()), (0.0, w), "{w}x{hgt}: the full width");
        assert_eq!(scene.top(), prompt.bottom() + SHELL_GAP, "{w}x{hgt}: right under the prompt row");
        assert_eq!(scene.bottom(), status.top(), "{w}x{hgt}: down to the status bar");
        assert_eq!(h.app.shell.content.unwrap().top(), scene.top(), "{w}x{hgt}: no band above the content");
        for name in ["browser:card", "document_bar:card", "history:card"] {
            let card = rect_of(name).unwrap_or_else(|| panic!("{w}x{hgt}: {name}"));
            assert!(scene.contains_rect(card), "{w}x{hgt}: {name} {card:?} floats inside the scene {scene:?}");
            assert!(card.left() >= SHELL_GAP && card.right() <= w - SHELL_GAP, "{w}x{hgt}: {name} is inset from the window's edges");
        }
        // The chrome: shorter than the kit's shell (48 + 7 + 141 + 7 + 36 + 7 = 246 points).
        let logo = rect_of("appbar:logo").unwrap();
        assert!(ribbon.top() - SHELL_GAP < 48.0, "{w}x{hgt}: the title bar is {} tall", ribbon.top() - SHELL_GAP);
        assert!(logo.width() < 32.0 && logo.height() < 32.0, "{w}x{hgt}: the logo is {:?}", logo.size());
        assert!(ribbon.height() < 141.0, "{w}x{hgt}: the ribbon card is {} tall", ribbon.height());
        assert!(scene.top() <= 210.0, "{w}x{hgt}: the chrome ends at {}", scene.top());
        // The canvas's share of the window: about 68 % on the laptop (whose fixed chrome is a
        // larger part of its height), about 76 % at the default size and 78 % at the kit's; the
        // kit shell gave it 44 %, 55 % and 58 %.
        let share = f64::from(scene.area()) / f64::from(w * hgt);
        assert!(share > 0.65, "{w}x{hgt}: the canvas is {:.1}% of the window", share * 100.0);
        if hgt >= 1000.0 {
            assert!(share > 0.75, "{w}x{hgt}: the canvas is {:.1}% of the window", share * 100.0);
        }
    }
}

/// Every pair of floating cards over the canvas stays apart at every size: the Design Browser,
/// the document strip, the history, Properties (or Review changes), the agent activity feed, a
/// toast, the navigation bar and the view cube.
#[test]
fn floating_cards_never_overlap_at_every_size() {
    for review in [false, true] {
        for (w, hgt) in STAGE_WINDOWS {
            let mut h = Harness::new();
            h.app.home.open = false;
            h.call("ui.resize", json!({"width": w, "height": hgt}));
            h.app.run("solid.box", json!({"length": 40, "width": 30, "height": 20})).unwrap();
            h.frames(2);
            let body = h.app.session.world_state().bodies[0].name.clone();
            if review {
                crate::review::propose(
                    &mut h.app,
                    vec![("timeline.edit".into(), json!({"feature": "Box1", "set": {"length": "50 mm"}}))],
                    Some("Longer box"),
                    crate::review::Source::Control,
                )
                .unwrap();
            } else {
                h.app.run("select.set", json!({"items": [{"type": "body", "name": body}]})).unwrap();
            }
            h.app.agent_feed.entries.push_back(crate::agent_activity::Entry { at: h.app.now, label: "Box".into(), outcome: "done".into(), ok: true });
            h.app.status = Some(("Viewport message".into(), crate::now_ms(), false));
            h.frames(4);
            h.app.status = Some(("Viewport message".into(), crate::now_ms(), false));
            h.frames(2);
            let mut cards: Vec<(&str, egui::Rect)> = Vec::new();
            for name in [
                "browser:card",
                "document_bar:card",
                "history:card",
                "toast",
                "nav:outer",
                "axis_view:outer",
                if review { "review:panel" } else { "inspector:card" },
            ] {
                cards.push((name, rect_of(name).unwrap_or_else(|| panic!("review={review} {w}x{hgt}: {name} is drawn"))));
            }
            cards.push(("agent activity", h.app.shell.agent_activity.unwrap_or_else(|| panic!("review={review} {w}x{hgt}: the feed is drawn"))));
            if review {
                // The canvas status line ("AI review / … / Proposed geometry shown") is laid out
                // against the strip like the other overlays, never under it.
                let status = rect_of("review:status").unwrap_or_else(|| panic!("{w}x{hgt}: the review's status line is drawn"));
                assert!(
                    status.is_positive() && h.app.viewport.rect.unwrap().contains_rect(status),
                    "{w}x{hgt}: the status line {status:?} is on the canvas"
                );
                cards.push(("review status line", status));
            }
            for (i, (a, ra)) in cards.iter().enumerate() {
                for (b, rb) in cards.iter().skip(i + 1) {
                    assert!(!ra.intersect(*rb).is_positive(), "review={review} {w}x{hgt}: {a} {ra:?} overlaps {b} {rb:?}");
                }
            }
        }
    }
}

/// The texts one more frame paints, each with its bounds (its galley at its anchor), the clip
/// rect it was painted under, whether it was cut short with an ellipsis and how many lines it
/// took. Empty texts are left out.
fn painted_texts(h: &mut Harness, window: egui::Vec2) -> Vec<(String, egui::Rect, egui::Rect, bool, usize)> {
    fn walk(shape: &egui::Shape, clip: egui::Rect, out: &mut Vec<(String, egui::Rect, egui::Rect, bool, usize)>) {
        match shape {
            egui::Shape::Text(t) if !t.galley.job.text.trim().is_empty() => {
                // The galley's rect is relative to its anchor (left, centre or right aligned).
                let rect = t.galley.rect.translate(t.pos.to_vec2());
                out.push((t.galley.job.text.clone(), rect, clip, t.galley.elided, t.galley.rows.len()));
            }
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, clip, out)),
            _ => {}
        }
    }
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, window);
    let time = h.ctx.input(|i| i.time) + 1.0 / 60.0;
    let mut out = h.ctx.run_ui(egui::RawInput { screen_rect: Some(screen), time: Some(time), ..Default::default() }, |ui| {
        h.app.logic(ui.ctx());
        h.app.ui(ui);
    });
    out.textures_delta.clear();
    let mut texts = Vec::new();
    out.shapes.iter().for_each(|s| walk(&s.shape, s.clip_rect, &mut texts));
    texts
}

/// On every surface of the shell, at 1366 × 768, 1600 × 1000 and 1760 × 1100, in English,
/// Japanese and Chinese: no text is cut short with an ellipsis, none runs past the rect it is
/// clipped to or outside its surface, no two texts of a surface are drawn one over the other,
/// and on the ribbon, in the Design Browser's rows, on the document strip, on the History card's
/// labels and on the status bar every text keeps to one line (the Properties card's title and
/// the review card's notes wrap by design; their rows are checked by
/// `no_label_of_an_editable_kind_is_elided_in_en_ja_or_zh`). With the sample design and its last
/// feature selected (the Properties card), the feature edited in place (the editor), and a
/// review proposed (the Review changes card). The interface's metrics changed with Geist
/// (PK-90.shell-r7); this is what holds the labels to their room.
#[test]
fn no_text_wraps_is_cut_or_overlaps_on_the_shell_at_any_stage_size_in_en_ja_or_zh() {
    for lang in ["en", "ja", "zh"] {
        for (w, hgt) in STAGE_WINDOWS {
            let window = egui::vec2(w, hgt);
            let mut h = Harness::new();
            h.app.home.open = false;
            h.app.preferences.language = lang.to_string();
            h.call("ui.resize", json!({"width": w, "height": hgt}));
            h.app.session.run_script(&polykrom_engine::sample::script()).unwrap();
            h.frames(3);
            let last = h.app.session.doc.features.last().map(|f| f.id).expect("the sample's features");
            let mut checked = 0;
            for state in ["selected", "editor", "review"] {
                match state {
                    "selected" => {
                        h.app.run("select.set", json!({"items": [{"type": "feature", "id": last}]})).unwrap();
                    }
                    "editor" => crate::design_properties::begin_edit(&mut h.app, last),
                    _ => {
                        h.app.run("select.set", json!({"items": []})).unwrap();
                        let steps = json!([{"command": "parameters.change", "params": {"name": "thickness", "expression": "10 mm"}}]);
                        let r = h.call(
                            "ui.review",
                            json!({"prompt": "Make the plate 10 mm thick", "propose": {"commands": steps, "label": "Plate 10 mm thick"}}),
                        );
                        assert_eq!(r["ok"], true, "{lang} {w}x{hgt}: {r}");
                    }
                }
                h.frames(3);
                let texts = painted_texts(&mut h, window);
                let case = format!("{lang} {w}x{hgt} {state}");
                let card = if state == "review" { "review:panel" } else { "properties:panel" };
                for (surface, one_line) in [
                    ("ribbon:card", true),
                    ("browser:card", true),
                    ("document_bar:card", true),
                    ("history:card", true),
                    ("status:bar", true),
                    (card, false),
                ] {
                    let region = rect_of(surface).unwrap_or_else(|| panic!("{case}: {surface} is drawn"));
                    // The texts of the surface, each with the part of it that shows: a card's
                    // body scrolls under its pinned actions and footer, so a row at the body's
                    // edge is cut by the scroll viewport by design, never sideways.
                    let inside: Vec<_> = texts
                        .iter()
                        .filter(|(_, r, _, _, _)| region.contains(r.center()))
                        .map(|(text, rect, clip, elided, rows)| (text, *rect, rect.intersect(*clip), *clip, *elided, *rows))
                        .filter(|(_, _, shown, _, _, _)| shown.is_positive())
                        .collect();
                    assert!(!inside.is_empty(), "{case}: {surface} {region:?} has texts");
                    for (text, rect, shown, clip, elided, rows) in &inside {
                        assert!(!elided, "{case}: {surface}: `{text}` is cut short");
                        // A field scrolls a long expression inside its own narrow clip by
                        // design; everything else (a label, a row, a value) is clipped only by
                        // its surface or the body's scroll viewport, as wide as the surface.
                        let field = clip.width() < region.width() - 40.0;
                        assert!(
                            field || (rect.left() >= clip.left() - 1.0 && rect.right() <= clip.right() + 1.0),
                            "{case}: {surface}: `{text}` {rect:?} runs past its clip {clip:?}"
                        );
                        assert!(region.expand(1.0).contains_rect(*shown), "{case}: {surface}: `{text}` {shown:?} runs outside {region:?}");
                        if one_line {
                            assert_eq!(*rows, 1, "{case}: {surface}: `{text}` wraps");
                        }
                        checked += 1;
                    }
                    for (i, (a, _, ra, ..)) in inside.iter().enumerate() {
                        for (b, _, rb, ..) in inside.iter().skip(i + 1) {
                            assert!(
                                !ra.shrink(0.5).intersect(rb.shrink(0.5)).is_positive(),
                                "{case}: {surface}: `{a}` {ra:?} is drawn over `{b}` {rb:?}"
                            );
                        }
                    }
                }
                if state == "review" {
                    crate::review::cancel(&mut h.app);
                    h.frames(2);
                }
            }
            assert!(checked > 60, "{lang} {w}x{hgt}: {checked} texts checked");
        }
    }
}
