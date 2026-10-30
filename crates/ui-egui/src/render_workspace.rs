//! Integrated product preview. Shares the installed device/pipelines, not Design's scene policy.

use crate::ui_kit::{Container, Edge, Surface};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

use egui::{Color32, Sense};
use polykrom_engine::doc::render_settings::RenderSetup;
use polykrom_engine::render::{
    Availability, Camera, ExecutorDescriptor, ExecutorId, ProductScene, RenderLimits, RenderMode, RenderOperation, RenderProfile, RenderRequest,
    ScratchLayout,
};
use serde_json::json;

use crate::gpu::{Frame, GpuScene, SceneSlot, ViewportCallback};
use crate::{SolveApp, theme::Tokens};

const MAX_GEOMETRY_BYTES: usize = 128 * 1024 * 1024;

#[cfg(test)]
#[path = "render_workspace_tests.rs"]
mod tests;

pub struct State {
    pub host: Option<ExecutorDescriptor>,
    pub image_host: Option<polykrom_engine::render::job::CpuHost>,
    pub view: ViewState,
    revision: u64,
    design_camera: Option<(Camera, bool)>,
    slot: SceneSlot,
    key: Option<u64>,
    scene_signature: Option<(u64, usize, u64)>,
    scene: Option<ProductScene>,
}

/// Small per-document editor state; GPU uploads and host capabilities stay outside tabs.
#[derive(Clone, Debug)]
pub struct ViewState {
    pub name: String,
    draft: RenderSetup,
    dirty: bool,
    job: Option<polykrom_engine::render::job::JobId>,
    completed: Option<polykrom_engine::render::job::JobId>,
    transparent: bool,
    overwrite: bool,
}

impl Default for ViewState {
    fn default() -> Self {
        Self { name: "Studio".into(), draft: RenderSetup::default(), dirty: false, job: None, completed: None, transparent: false, overwrite: false }
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            host: None,
            image_host: None,
            view: Default::default(),
            revision: u64::MAX,
            design_camera: None,
            slot: Arc::new(Mutex::new(None)),
            key: None,
            scene_signature: None,
            scene: None,
        }
    }
}

pub fn active(app: &SolveApp) -> bool {
    app.ui.tab == "RENDER"
}

pub fn host_executor(max_dimension: u32, depth: bool, samples: u32) -> Result<ExecutorDescriptor, String> {
    if !matches!(samples, 1 | 2 | 4 | 8 | 16) {
        return Err("unsupported GPU sample count".into());
    }
    Ok(ExecutorDescriptor {
        id: ExecutorId::new("gpu-viewport").map_err(|e| e.to_string())?,
        availability: if depth && max_dimension > 0 {
            Availability::Available
        } else {
            Availability::Unavailable { reason: "product preview requires an installed device with a depth buffer".into() }
        },
        profiles: vec![RenderProfile {
            mode: RenderMode::Preview,
            operation: RenderOperation::Viewport,
            formats: Vec::new(),
            features: vec![polykrom_engine::render::RenderFeature::Pbr],
        }],
        limits: RenderLimits { max_dimension: u64::from(max_dimension.clamp(1, 4096)), ..Default::default() },
        scratch: ScratchLayout {
            sample_scale: 1,
            sample_bytes_per_pixel: 8 * u64::from(samples),
            output_bytes_per_pixel: 8,
            encoding_bytes_per_pixel: 0,
        },
    })
}

pub fn sync_backend(app: &mut SolveApp) {
    if app.session.render_job_host().is_none()
        && let Some(host) = &app.render_workspace.image_host
        && let Err(e) = app.session.set_render_job_host(Some(host.clone()))
    {
        app.set_status(e.to_string(), true);
    }
    if app.session.render_viewport() == app.render_workspace.host.as_ref() {
        return;
    }
    if let Err(e) = app.session.set_render_viewport(app.render_workspace.host.clone()) {
        app.set_status(e.to_string(), true);
    }
}

/// Cooperative work belongs to the frame loop, never to a status/control request. Keep
/// scheduling frames even after leaving Render so a browser job can finish in Design.
pub fn tick(app: &mut SolveApp, ctx: &egui::Context) {
    if let Some(host) = &app.render_workspace.image_host {
        match host.pump(1024) {
            Ok(true) => ctx.request_repaint(),
            Ok(false) => {}
            Err(e) => app.set_status(e.to_string(), true),
        }
    }
}

pub fn reset_document(app: &mut SolveApp) {
    app.ui.ribbon_context = None;
    let host = app.render_workspace.host.clone();
    let image_host = app.render_workspace.image_host.clone();
    app.render_workspace = State { host, image_host, ..Default::default() };
    if active(app) {
        app.ui.tab = "MODEL".into();
    }
    sync_backend(app);
}

pub fn invalidate_scene(app: &mut SolveApp) {
    app.render_workspace.revision = u64::MAX;
    app.render_workspace.scene_signature = None;
    app.render_workspace.scene = None;
    app.render_workspace.key = None;
}

pub fn enter(app: &mut SolveApp, render: bool) -> Result<(), String> {
    if render {
        if app.session.active_sketch.is_some() || app.tool.is_some() || app.dialog.is_some() || app.file_load.busy() {
            return Err("finish the current modelling operation before entering Render".into());
        }
        app.run("render.scene", json!({}))?;
        if !active(app) {
            app.render_workspace.design_camera = Some((app.cam, app.ui.perspective));
            app.ui.tab = "RENDER".into();
            let scene = polykrom_engine::product_scene::extract(&app.session).map_err(|e| e.to_string())?;
            if !scene.bounds.is_empty() {
                app.cam.fit(&scene.bounds);
            }
        }
    } else {
        if let Some((camera, perspective)) = app.render_workspace.design_camera.take() {
            app.cam = camera;
            app.ui.up_axis = camera.up;
            app.ui.perspective = perspective;
        }
        app.ui.tab = "MODEL".into();
    }
    app.cancel_view_animation();
    Ok(())
}

pub fn panel(app: &mut SolveApp, ui: &mut egui::Ui) {
    if app.render_workspace.revision != app.session.revision && !app.render_workspace.view.dirty {
        app.render_workspace.view.draft = app.session.doc.render.setups.get(&app.render_workspace.view.name).cloned().unwrap_or_default();
        app.render_workspace.revision = app.session.revision;
    }
    let response = Container::new(ui.ctx(), Surface::Panel)
        .edge("sc_render_settings", Edge::Right)
        .exact_size(285.0)
        .resizable(false)
        .frame(Container::new(ui.ctx(), Surface::Panel).edge_frame(Tokens::get().panel).inner_margin(10))
        .show(ui, |ui| {
            crate::ui_kit::scope(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let t = Tokens::get();
                    ui.label(
                        crate::fonts::RichTextExt::semibold(egui::RichText::new(app.tr("cmd.render_settings_get")))
                            .color(t.text)
                            .size(t.font_heading),
                    );
                    crate::ui_kit::text(ui, app.tr("render.preview_summary"), crate::ui_kit::Text::Note("render_summary"))
                        .on_hover_text(app.tr("render.opaque_preview_help"));
                    scene_section(app, ui);
                    camera_section(app, ui);
                    crate::ui_kit::section(ui, "render_output", app.tr("render.section_output"), true, |ui| output_section(app, ui));
                    ui.add_space(t.space_8);
                    image_controls(app, ui);
                    ui.add_space(t.space_8);
                    crate::ui_kit::divider(ui, "render_footer");
                    if crate::ui_kit::button(ui, app.tr("render.return_design"), crate::ui_kit::Button::Normal).clicked()
                        && let Err(e) = enter(app, false)
                    {
                        app.set_status(e, true);
                    }
                });
            });
        });
    crate::scenario::publish_rect("render:settings", response.response.rect);
    let frame = Container::new(ui.ctx(), Surface::Panel).edge_frame(Tokens::get().panel).inner_margin(10);
    Container::paint_inset(ui.painter(), response.response.rect - frame.outer_margin, frame);
}

/// Two equal-width secondary actions on one row, or stacked full width when a translation would
/// not fit half the row; returns which was clicked.
/// Tokens: `form_button_height, space_4/6/12, font_label`; the shared button component draws them.
fn action_pair(ui: &mut egui::Ui, id: &str, actions: [(&str, &str, bool); 2]) -> [bool; 2] {
    let t = Tokens::get();
    let full = ui.available_width().max(0.0);
    let half = ((full - t.space_6) / 2.0).max(0.0);
    let fits = actions.iter().all(|(label, ..)| {
        ui.painter().layout_no_wrap((*label).to_owned(), egui::FontId::proportional(t.font_label), t.form_ink).size().x + 2.0 * t.space_12 <= half
    });
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(full, if fits { t.form_button_height } else { 2.0 * t.form_button_height + t.space_4 }), Sense::hover());
    let mut clicked = [false; 2];
    for (index, (label, tip, enabled)) in actions.into_iter().enumerate() {
        let i = index as f32;
        let at = if fits {
            egui::Rect::from_min_size(egui::pos2(rect.left() + i * (half + t.space_6), rect.top()), egui::vec2(half, t.form_button_height))
        } else {
            egui::Rect::from_min_size(
                egui::pos2(rect.left(), rect.top() + i * (t.form_button_height + t.space_4)),
                egui::vec2(full, t.form_button_height),
            )
        };
        let response = crate::ui_kit::button_at(ui, &format!("{id}:{index}"), at, label, crate::ui_kit::Button::Enabled(enabled));
        if let Some(slot) = clicked.get_mut(index) {
            *slot = response.on_hover_text(tip).clicked();
        }
    }
    clicked
}

fn scene_section(app: &mut SolveApp, ui: &mut egui::Ui) {
    crate::ui_kit::section(ui, "render_scene", app.tr("render.section_scene"), true, |ui| {
        let names: Vec<_> = app.session.doc.render.setups.keys().cloned().collect();
        let old_name = app.render_workspace.view.name.clone();
        crate::ui_kit::row(ui, app.tr("render.setup"), |ui| {
            crate::ui_kit::dropdown_menu(ui, "render_setups", &old_name, |ui| {
                for name in names {
                    if crate::ui_kit::list_picker(ui, &name, &name, name == app.render_workspace.view.name).clicked() {
                        app.render_workspace.view.name = name;
                        ui.close();
                    }
                }
            });
        });
        crate::ui_kit::row(ui, app.tr("render.setup_name"), |ui| {
            crate::ui_kit::bounded_name_field(ui, ui.id().with("render_setup_name"), &mut app.render_workspace.view.name, "", 128);
        });
        if app.render_workspace.view.name != old_name {
            app.render_workspace.view.draft = app.session.doc.render.setups.get(&app.render_workspace.view.name).cloned().unwrap_or_default();
            app.render_workspace.view.dirty = false;
        }
        crate::ui_kit::row(ui, app.tr("render.background"), |ui| {
            if crate::ui_kit::color_field(ui, &mut app.render_workspace.view.draft.background).changed() {
                app.render_workspace.view.dirty = true;
            }
        });
        if themed_backdrop(&app.render_workspace.view.draft).is_some() {
            crate::ui_kit::text(ui, app.tr("render.backdrop_caption"), crate::ui_kit::Text::Note("render_backdrop"));
        }
        if app.render_workspace.view.draft.backend.as_str() != "builtin" || app.render_workspace.view.draft.mode != RenderMode::Preview {
            crate::ui_kit::text(ui, app.tr("render.unavailable_preview"), crate::ui_kit::Text::Note("render_unavailable"));
            if crate::ui_kit::button(ui, app.tr("render.use_preview"), crate::ui_kit::Button::Normal).clicked() {
                app.render_workspace.view.draft.backend = polykrom_engine::render::builtin_backend().id;
                app.render_workspace.view.draft.mode = RenderMode::Preview;
                app.render_workspace.view.dirty = true;
            }
        }
        let [save, reload] = action_pair(
            ui,
            "render_setup_actions",
            [
                (app.tr("render.save_setup"), app.tr("render.save_setup_help"), true),
                (app.tr("render.reload_settings"), app.tr("render.reload_help"), true),
            ],
        );
        if save {
            let name = app.render_workspace.view.name.clone();
            let setup = RenderSetup { camera: app.cam, ..app.render_workspace.view.draft.clone() };
            if app.run("render.settings.set", json!({"name":name,"set":setup})).is_ok() {
                app.render_workspace.view.dirty = false;
            }
        }
        if reload {
            app.render_workspace.view.dirty = false;
            app.render_workspace.revision = u64::MAX;
        }
        if app.render_workspace.view.dirty {
            crate::ui_kit::text(ui, app.tr("render.unsaved"), crate::ui_kit::Text::Note("render_unsaved"));
        }
    });
}

fn camera_section(app: &mut SolveApp, ui: &mut egui::Ui) {
    crate::ui_kit::section(ui, "render_camera", app.tr("render.section_camera"), true, |ui| {
        let mut perspective = app.cam.fov > 0.0;
        if crate::ui_kit::checkbox(ui, app.tr("render.perspective"), &mut perspective).changed() {
            app.cam.fov = if perspective { std::f64::consts::FRAC_PI_4 } else { 0.0 };
            app.render_workspace.view.dirty = true;
        }
        let [fit, restore] = action_pair(
            ui,
            "render_camera_actions",
            [(app.tr("render.fit"), app.tr("render.fit_help"), true), (app.tr("render.restore_camera"), app.tr("render.restore_help"), true)],
        );
        if fit {
            match polykrom_engine::product_scene::extract(&app.session) {
                Ok(scene) if !scene.bounds.is_empty() => app.cam.fit(&scene.bounds),
                Err(e) => app.set_status(e.to_string(), true),
                _ => {}
            }
        }
        if restore {
            let name = app.render_workspace.view.name.clone();
            if app.run("render.settings.apply", json!({"name":name})).is_ok() {
                app.follow_view_request(false);
            }
        }
        crate::ui_kit::text(ui, app.tr("render.navigation_short"), crate::ui_kit::Text::Note("render_navigation"))
            .on_hover_text(app.tr("render.navigation_help"));
    });
}

fn output_section(app: &mut SolveApp, ui: &mut egui::Ui) {
    for (label, height) in [("render.image_width", false), ("render.image_height", true)] {
        crate::ui_kit::row(ui, app.tr(label), |ui| {
            // Equal-width size fields, whatever their digits.
            ui.spacing_mut().interact_size.x = Tokens::get().form_label_width + Tokens::get().space_24;
            let draft = &mut app.render_workspace.view.draft;
            let value = if height { &mut draft.height } else { &mut draft.width };
            if crate::ui_kit::number_field(ui, value, 1..=4096, 1.0, " px").changed() {
                app.render_workspace.view.dirty = true;
            }
        });
    }
    let draft = &app.render_workspace.view.draft;
    let size = (u64::from(draft.width), u64::from(draft.height));
    if let Some((w, h)) = RenderLimits::default().fit(size.0, size.1, None)
        && (w, h) != size
    {
        crate::ui_kit::text(ui, app.tr("render.image_too_large"), crate::ui_kit::Text::Note("render_too_large"));
        if crate::ui_kit::button(ui, format!("{} {w} × {h}", app.tr("render.image_use_largest")), crate::ui_kit::Button::Normal).clicked() {
            app.render_workspace.view.draft.width = u32::try_from(w).unwrap_or(1);
            app.render_workspace.view.draft.height = u32::try_from(h).unwrap_or(1);
            app.render_workspace.view.dirty = true;
        }
    }
    crate::ui_kit::checkbox(ui, app.tr("render.image_transparent"), &mut app.render_workspace.view.transparent);
    crate::ui_kit::checkbox(ui, app.tr("render.image_overwrite"), &mut app.render_workspace.view.overwrite);
}

/// The one primary action: Render, replaced in place by progress and Cancel while a job runs,
/// then the finished image's status and Export.
fn image_controls(app: &mut SolveApp, ui: &mut egui::Ui) {
    let t = Tokens::get();
    let mut busy = false;
    let mut status_line = None;
    if let Some(job) = app.render_workspace.view.job {
        match app.session.execute("render.status", &json!({"job":job})) {
            Ok(status) => {
                let phase = status["progress"]["phase"].as_str().unwrap_or("unknown");
                busy = matches!(phase, "queued" | "preparing" | "running" | "encoding" | "cancelling");
                let text = match phase {
                    "queued" => app.tr("render.image_queued"),
                    "preparing" => app.tr("render.image_preparing"),
                    "running" => app.tr("render.image_running"),
                    "encoding" => app.tr("render.image_encoding"),
                    "cancelling" => app.tr("render.image_cancelling"),
                    "cancelled" => app.tr("render.image_cancelled"),
                    "completed" => app.tr("render.image_completed"),
                    "failed" => app.tr("render.image_failed"),
                    _ => app.tr("render.image_unknown"),
                };
                if phase == "completed" {
                    app.render_workspace.view.completed = Some(job);
                }
                if busy {
                    let done = status["progress"]["done"].as_u64().unwrap_or(0);
                    let total = status["progress"]["total"].as_u64().unwrap_or(1).max(1);
                    let fraction = (done as f64 / total as f64).clamp(0.0, 1.0) as f32;
                    // One row, the primary button's height: the step and percentage over the bar.
                    let (row, _) = ui.allocate_exact_size(egui::vec2(ui.available_width().max(0.0), t.form_button_height), Sense::hover());
                    let cancel = app.tr("cmd.render_cancel");
                    let cancel_width = ui.painter().layout_no_wrap(cancel.to_owned(), egui::FontId::proportional(t.font_label), t.form_ink).size().x
                        + 2.0 * t.space_12;
                    let cancel_rect = egui::Rect::from_min_max(egui::pos2((row.right() - cancel_width).max(row.left()), row.top()), row.max);
                    let left = egui::Rect::from_min_max(row.min, egui::pos2((cancel_rect.left() - t.space_8).max(row.left()), row.bottom()));
                    let painter = ui.painter().with_clip_rect(ui.clip_rect().intersect(left));
                    let font = egui::FontId::proportional(t.font_small);
                    painter.text(left.left_top(), egui::Align2::LEFT_TOP, text, font.clone(), t.form_ink);
                    painter.text(left.right_top(), egui::Align2::RIGHT_TOP, format!("{:.0}%", fraction * 100.0), font, t.form_dim);
                    let bar = egui::Rect::from_min_max(egui::pos2(left.left(), left.bottom() - t.space_6), left.max);
                    ui.scope_builder(egui::UiBuilder::new().max_rect(bar), |ui| crate::ui_kit::progress(ui, "render", fraction))
                        .inner
                        .on_hover_text(text);
                    if crate::ui_kit::button_at(ui, "render_cancel", cancel_rect, cancel, crate::ui_kit::Button::Normal).clicked() {
                        let _ = app.run("render.cancel", json!({"job":job}));
                    }
                    ui.ctx().request_repaint();
                } else {
                    status_line = Some((text.to_string(), phase == "failed"));
                }
                if let Some(message) = status["error"]["message"].as_str() {
                    let reduce =
                        if status["error"]["code"] == "limit_exceeded" { format!(" {}", app.tr("render.image_reduce")) } else { String::new() };
                    status_line = Some((format!("{}: {message}{reduce}", status_line.map_or(text.to_string(), |(s, _)| s)), true));
                }
                if let Some(revision) = status["scene_generation"].as_u64()
                    && revision != app.session.revision
                {
                    crate::ui_kit::text(ui, app.tr("render.image_stale"), crate::ui_kit::Text::Note("render_stale"));
                }
            }
            Err(e) => status_line = Some((e.to_string(), true)),
        }
    }
    let centred = |ui: &mut egui::Ui, add: &mut dyn FnMut(&mut egui::Ui) -> egui::Response| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width().max(0.0), t.form_button_height), Sense::hover());
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight)), add).inner
    };
    let start = (!busy).then(|| centred(ui, &mut |ui| crate::ui_kit::wide_primary_button(ui, app.tr("render.image_start"))));
    if let Some(start) = &start {
        crate::scenario::publish_target("render:start", start.rect);
    }
    if start.is_some_and(|start| start.clicked()) {
        let settings = RenderSetup { camera: app.cam, ..app.render_workspace.view.draft.clone() };
        if let Ok(reply) = app.run("render.start", json!({"settings":settings,"transparent_background":app.render_workspace.view.transparent})) {
            app.render_workspace.view.job = serde_json::from_value(reply["job"].clone()).ok();
            ui.ctx().request_repaint();
        }
    }
    match status_line {
        Some((text, true)) => {
            crate::ui_kit::text(ui, &text, crate::ui_kit::Text::Report("render_status", true));
        }
        Some((text, false)) => {
            crate::ui_kit::text(ui, &text, crate::ui_kit::Text::Note("render_status"));
        }
        None => {}
    }
    let completed = app.render_workspace.view.completed;
    let export = app.tr("render.image_export");
    if centred(ui, &mut |ui| crate::ui_kit::button(ui, export, crate::ui_kit::Button::Enabled(completed.is_some()))).clicked()
        && let Some(job) = completed
    {
        export_image(app, job, app.render_workspace.view.overwrite);
    }
}

/// Keep the selected image and overwrite consent fixed while the upstream picker holds model input.
fn export_image(app: &mut SolveApp, job: polykrom_engine::render::job::JobId, overwrite: bool) {
    let origin = app.session.doc.clone();
    app.pick_save("render.png", &["png"], move |app, path| {
        let Some(path) = path else { return };
        if !Arc::ptr_eq(&origin, &app.session.doc) {
            app.set_status("The render export document changed while choosing its destination", true);
            return;
        }
        let _ = app.run("render.export", json!({"job":job,"path":path,"overwrite":overwrite}));
    });
}

/// Convert only product surfaces; geometry budgets are admitted before vertex allocation.
pub fn gpu_scene(scene: &ProductScene) -> Result<(u64, GpuScene), String> {
    let bytes = scene
        .meshes
        .iter()
        .try_fold(0usize, |n, m| m.mesh.triangles.len().checked_mul(3 * crate::gpu::PBR_SIZE).and_then(|x| n.checked_add(x)))
        .ok_or("product geometry size overflow")?;
    if bytes > MAX_GEOMETRY_BYTES {
        return Err("product preview exceeds the 128 MiB geometry upload budget".into());
    }
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    let mut gpu = GpuScene::default();
    for body in &scene.meshes {
        if body.faces.len() > 262_144 {
            return Err("product preview exceeds the face-appearance admission cap".into());
        }
        if !body.opacity.is_finite()
            || !(0.0..=1.0).contains(&body.opacity)
            || body.faces.iter().any(|f| !f.opacity.is_finite() || !(0.0..=1.0).contains(&f.opacity))
        {
            return Err("invalid product appearance opacity".into());
        }
        (Arc::as_ptr(&body.mesh) as usize).hash(&mut hash);
        body.name.hash(&mut hash);
        body.color.hash(&mut hash);
        body.opacity.to_bits().hash(&mut hash);
        hash_surface(body.surface, &mut hash);
        let looks: std::collections::BTreeMap<_, _> = body.faces.iter().map(|l| (l.face, l)).collect();
        for face in &body.faces {
            face.face.hash(&mut hash);
            face.color.hash(&mut hash);
            face.opacity.to_bits().hash(&mut hash);
            hash_surface(face.surface, &mut hash);
        }
        for (index, triangle) in body.mesh.triangles.iter().enumerate() {
            let face = body.mesh.tri_face.get(index).and_then(|f| looks.get(f));
            let (color, opacity, surface) = face.map_or((body.color, body.opacity, body.surface), |l| (l.color, l.opacity, l.surface));
            if surface.is_some() && opacity != 1.0 {
                return Err("opaque PBR requires opacity 1".into());
            }
            if opacity <= 0.0 {
                continue;
            }
            let vertices = triangle.map(|i| {
                let p = body.mesh.positions.get(i as usize)?.to_f32();
                let n = *body.mesh.normals.get(i as usize)?;
                let n = if surface.is_some() { polykrom_engine::render::surface::unit_direction(n).ok()? } else { n }.to_f32();
                p.into_iter().chain(n).all(f32::is_finite).then_some((p, n))
            });
            let [Some(a), Some(b), Some(c)] = vertices else { return Err("invalid product triangle".into()) };
            let rgba = [color[0], color[1], color[2], (opacity * 255.0).round() as u8];
            let (buffer, stride) = if surface.is_some() {
                (&mut gpu.pbr, crate::gpu::PBR_SIZE)
            } else if opacity < 0.999 {
                (&mut gpu.glass, crate::gpu::TRI_SIZE)
            } else {
                (&mut gpu.tris, crate::gpu::TRI_SIZE)
            };
            buffer.try_reserve(3 * stride).map_err(|_| "product preview vertex allocation failed".to_string())?;
            for (p, n) in [a, b, c] {
                if let Some(surface) = surface {
                    if n.iter().all(|v| *v == 0.0) {
                        return Err("PBR requires nonzero surface normals".into());
                    }
                    gpu.pbr_tri(p, n, surface);
                } else if opacity < 0.999 {
                    gpu.glass_tri(p, n, rgba);
                } else {
                    gpu.tri(p, n, rgba);
                }
            }
        }
    }
    Ok((hash.finish(), gpu))
}

fn hash_surface(surface: Option<polykrom_engine::render::surface::OpaqueSurface>, hash: &mut impl Hasher) {
    surface.is_some().hash(hash);
    if let Some(s) = surface {
        let c = s.base_color();
        for value in [c.x, c.y, c.z, s.metallic(), s.roughness(), s.dielectric_ior()] {
            value.to_bits().hash(hash);
        }
    }
}

/// The preview's backdrop when it is not the setup's background: in a dark theme an untouched
/// default background previews on the theme's viewport colour, not a light sheet beside the dark
/// UI. A picked or saved colour shows as it is, and rendered images always use the setup's.
pub(crate) fn themed_backdrop(setup: &RenderSetup) -> Option<Color32> {
    (crate::theme::is_dark() && setup.background == RenderSetup::default().background).then(|| Tokens::get().viewport_top)
}

/// The size the product preview draws at for a viewport of `full` physical pixels: the full
/// size when the viewport executor admits it, else the largest size at the same aspect it does.
/// Without an executor the request is left for resolution to report.
pub fn preview_size(executor: Option<&ExecutorDescriptor>, full: (u64, u64)) -> (u64, u64) {
    executor.and_then(|e| e.fit(full.0, full.1)).unwrap_or(full)
}

pub fn show(app: &mut SolveApp, ui: &mut egui::Ui) {
    let rect = ui.max_rect();
    app.viewport.rect = Some(rect);
    crate::viewport::refresh_layout(app, ui.ctx().pixels_per_point());
    let response = ui.interact(rect, ui.id().with("render_preview"), Sense::click_and_drag());
    // The modelling viewport's camera input (preset, Orbit around, zoom to the cursor, middle
    // double-click), with nothing to pick or select here.
    crate::navigation::handle(app, ui, rect, &response, crate::navigation::NavCaps::RENDER);
    let bg = app.render_workspace.view.draft.background;
    let backdrop = themed_backdrop(&app.render_workspace.view.draft).unwrap_or(Color32::from_rgb(bg[0], bg[1], bg[2]));
    ui.painter().rect_filled(rect, 0.0, backdrop);
    let result = (|| -> Result<(), String> {
        if app.viewport.gpu.is_none() {
            return Err("GPU product preview is unavailable on this host. Saved setups remain available.".into());
        }
        let ppp = ui.ctx().pixels_per_point();
        let setup = &app.render_workspace.view.draft;
        if setup.exposure_ev != 0.0 || setup.environment_rotation != 0.0 || setup.environment_intensity != 1.0 {
            return Err("fixed studio preview requires exposure 0, environment rotation 0 and intensity 1".into());
        }
        // A 4K or 5K viewport is past the preview's pixel budget: draw it smaller and filter it
        // up into the viewport rather than refusing the frame.
        let full = ((rect.width() * ppp).max(1.0) as u64, (rect.height() * ppp).max(1.0) as u64);
        let (width, height) = preview_size(app.session.render_viewport(), full);
        let mut request = RenderRequest {
            backend: Some(setup.backend.clone()),
            mode: setup.mode,
            operation: RenderOperation::Viewport,
            width,
            height,
            format: None,
            features: Vec::new(),
            policy: Default::default(),
        };
        polykrom_engine::render::resolve_request(&[app.session.render_backend()], &request).map_err(|e| e.to_string())?;
        if app.session.model.is_stale() {
            return Err("finish the modelling gesture before previewing".into());
        }
        let mut visibility = std::collections::hash_map::DefaultHasher::new();
        app.session.visibility.hidden_bodies.hash(&mut visibility);
        app.session.visibility.hidden_occurrences.hash(&mut visibility);
        let signature = (app.session.revision, Arc::as_ptr(&app.session.world_state()) as usize, visibility.finish());
        if app.render_workspace.scene_signature != Some(signature) {
            let scene = polykrom_engine::product_scene::extract(&app.session).map_err(|e| e.to_string())?;
            let (key, gpu) = gpu_scene(&scene)?;
            if app.render_workspace.key != Some(key) {
                if let Ok(mut slot) = app.render_workspace.slot.lock() {
                    *slot = Some(gpu);
                }
                app.render_workspace.key = Some(key);
            }
            app.render_workspace.scene = Some(scene);
            app.render_workspace.scene_signature = Some(signature);
        }
        let scene = app.render_workspace.scene.as_ref().ok_or("no product scene")?;
        request.features = scene.required_features();
        polykrom_engine::render::resolve_request(&[app.session.render_backend()], &request).map_err(|e| e.to_string())?;
        let key = app.render_workspace.key.ok_or("no product geometry")?;
        if scene.bounds.is_empty() {
            return Err("Create a visible solid in Design to preview it here.".into());
        }
        let camera = app.cam;
        let radius = scene.bounds.diagonal().max(10.0) + scene.bounds.center().dist(camera.target);
        let eye = camera.eye();
        let slot = app.render_workspace.slot.clone();
        let callback = ViewportCallback(Frame {
            product: true,
            key,
            slot: slot.clone(),
            sketch_key: 0,
            sketch_slot: slot.clone(),
            hl_key: 0,
            hl_slot: slot.clone(),
            pv_key: 0,
            pv_slot: slot,
            pv_hide: false,
            view_proj: camera.view_proj(f64::from(rect.width() / rect.height().max(1.0)), radius).to_f32(),
            back: camera.back().to_f32(),
            size_px: [rect.width() * ppp, rect.height() * ppp],
            clip: [0.0; 4],
            cap: [0.0; 4],
            images: Vec::new(),
            analysis: [0.0; 8],
            analysis_colors: [0.0; 36],
            access: None,
            outline: false,
            // Product rendering has its own lighting; AO is a Design viewport setting.
            ao_radius: 0.0,
            eye: [eye.x as f32, eye.y as f32, eye.z as f32, if camera.fov > 0.0 { 1.0 } else { 0.0 }],
            ssaa: if width < full.0 { width as f32 / full.0 as f32 } else { 1.0 },
            hover_tint: [0.0; 3],
            selection_tint: [0.0; 3],
        });
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(rect, callback));
        Ok(())
    })();
    if let Err(error) = result {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            error,
            egui::FontId::proportional(crate::theme::Tokens::get().font_heading),
            Tokens::get().text,
        );
    }
    crate::viewport::axis_view(app, ui, rect);
}
