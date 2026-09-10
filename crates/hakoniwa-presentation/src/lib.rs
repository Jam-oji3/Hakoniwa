use eframe::egui;
use hakoniwa_domain::{Bead, Color, GridPosition, Piece, Plane, Project};
use std::{collections::BTreeSet, sync::Arc};
pub struct HakoniwaApp {
    project: Project,
    selected: Option<u64>,
    status: String,
    yaw: f32,
    zoom: f32,
    pitch: f32,
    pan: egui::Vec2,
    perspective: bool,
}
impl HakoniwaApp {
    fn new(ctx: &egui::Context) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "noto-jp".into(),
            Arc::new(egui::FontData::from_static(include_bytes!(
                "../../../assets/NotoSansCJKjp-Regular.otf"
            ))),
        );
        fonts
            .families
            .get_mut(&egui::FontFamily::Proportional)
            .unwrap()
            .insert(0, "noto-jp".into());
        fonts
            .families
            .get_mut(&egui::FontFamily::Monospace)
            .unwrap()
            .insert(0, "noto-jp".into());
        ctx.set_fonts(fonts);
        ctx.set_visuals(egui::Visuals::light());
        Self {
            project: hammer_project(),
            selected: None,
            status: "ハンマーのMVPサンプルを読み込みました".into(),
            yaw: 45.0,
            zoom: 1.0,
            pitch: 25.0,
            pan: egui::Vec2::ZERO,
            perspective: true,
        }
    }
}
fn hammer_project() -> Project {
    let mut project = Project::new("ハンマー");
    let head = project.create_shape("頭部");
    for x in -3..=3 {
        for y in -1..=1 {
            project
                .add_shape_bead(
                    head,
                    Bead {
                        position: GridPosition::new(x, y, 0),
                        color: Color::RED,
                    },
                )
                .unwrap();
        }
    }
    project
        .convert_shape_plane_to_piece(head, Plane::Xy { z: 0 }, "頭部")
        .unwrap();
    let handle = project.create_shape("柄");
    for y in -6..=0 {
        project
            .add_shape_bead(
                handle,
                Bead {
                    position: GridPosition::new(0, y, 1),
                    color: Color::BROWN,
                },
            )
            .unwrap();
    }
    project
        .convert_shape_plane_to_piece(handle, Plane::Xy { z: 1 }, "柄")
        .unwrap();
    project
}
impl eframe::App for HakoniwaApp {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        ui.painter()
            .rect_filled(ui.max_rect(), 0.0, egui::Color32::WHITE);
        ui.horizontal(|ui| {
            if ui.button("ハンマーを新規作成").clicked() {
                self.project = hammer_project();
                self.selected = None;
                self.status = "ハンマーを作成しました".into();
            }
            ui.label(&self.status);
        });
        ui.separator();
        let panel_rect = ui.available_rect_before_wrap();
        let boundary_color = egui::Color32::from_gray(180);
        let first_boundary = panel_rect.left() + panel_rect.width() / 3.0;
        let second_boundary = panel_rect.left() + panel_rect.width() * 2.0 / 3.0;
        ui.painter().line_segment(
            [
                egui::pos2(first_boundary, panel_rect.top()),
                egui::pos2(first_boundary, panel_rect.bottom()),
            ],
            egui::Stroke::new(1.0, boundary_color),
        );
        ui.painter().line_segment(
            [
                egui::pos2(second_boundary, panel_rect.top()),
                egui::pos2(second_boundary, panel_rect.bottom()),
            ],
            egui::Stroke::new(1.0, boundary_color),
        );
        ui.columns(3, |columns| {
            columns[0].heading("パーツツリー");
            for (id, piece) in &self.project.pieces {
                if columns[0]
                    .selectable_label(
                        self.selected == Some(*id),
                        format!("▦ {} ({} beads)", piece.name, piece.beads.len()),
                    )
                    .clicked()
                {
                    self.selected = Some(*id);
                }
            }
            columns[0].separator();
            columns[0].label(format!("総ビーズ数: {}", self.project.inventory().total));
            draw_editor(
                &mut columns[1],
                self.project.pieces.get(&self.selected.unwrap_or(0)),
            );
            draw_preview(
                &mut columns[2],
                &self.project,
                &mut self.yaw,
                &mut self.zoom,
                &mut self.pitch,
                &mut self.pan,
                &mut self.perspective,
            );
        });
    }
}
fn draw_editor(ui: &mut egui::Ui, piece: Option<&Piece>) {
    ui.heading("2D Piece エディタ");
    let Some(piece) = piece else {
        ui.label("ツリーからPieceを選択してください");
        return;
    };
    ui.label(format!("{} · {:?}", piece.name, piece.plane));
    let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, egui::Color32::from_gray(250));
    for i in 0..11 {
        let x = rect.left() + i as f32 * 30.0;
        let y = rect.top() + i as f32 * 30.0;
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            egui::Stroke::new(1.0, egui::Color32::from_gray(190)),
        );
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            egui::Stroke::new(1.0, egui::Color32::from_gray(190)),
        );
    }
    for (pos, color) in &piece.beads {
        painter.circle_filled(
            egui::pos2(
                rect.center().x + pos.x as f32 * 30.0,
                rect.center().y - pos.y as f32 * 30.0,
            ),
            11.0,
            egui::Color32::from_rgb(color.0, color.1, color.2),
        );
    }
}
fn draw_preview(
    ui: &mut egui::Ui,
    project: &Project,
    yaw: &mut f32,
    zoom: &mut f32,
    pitch: &mut f32,
    pan: &mut egui::Vec2,
    perspective: &mut bool,
) {
    ui.heading("3D Assembly ビュー");
    ui.checkbox(perspective, "透視投影");
    ui.horizontal(|ui| {
        ui.label("カメラ回転");
        ui.add(egui::Slider::new(yaw, 0.0..=360.0).suffix("°"));
    });
    ui.horizontal(|ui| {
        ui.label("ズーム");
        ui.add(egui::Slider::new(zoom, 0.5..=2.0));
    });
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::drag());
    if response.hovered() {
        ui.input(|input| {
            if input.pointer.button_down(egui::PointerButton::Middle) {
                if input.modifiers.shift {
                    *pan += input.pointer.delta();
                } else {
                    *yaw = (*yaw + input.pointer.delta().x * 0.5).rem_euclid(360.0);
                    *pitch = (*pitch - input.pointer.delta().y * 0.5).clamp(-85.0, 85.0);
                }
            }
            if input.smooth_scroll_delta.y != 0.0 {
                *zoom = (*zoom * (1.0 + input.smooth_scroll_delta.y * 0.001)).clamp(0.5, 2.0);
            }
        });
    }
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, egui::Color32::from_gray(244));
    let occupied = project
        .pieces
        .values()
        .flat_map(|piece| piece.beads.keys().copied())
        .collect::<BTreeSet<_>>();
    for piece in project.pieces.values() {
        for (position, color) in &piece.beads {
            draw_voxel(
                painter,
                rect.center(),
                *position,
                *color,
                *yaw,
                *zoom,
                *pitch,
                *pan,
                *perspective,
                &occupied,
            );
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn project(
    center: egui::Pos2,
    x: f32,
    y: f32,
    z: f32,
    yaw: f32,
    pitch: f32,
    zoom: f32,
    pan: egui::Vec2,
    perspective: bool,
) -> egui::Pos2 {
    let yaw = yaw.to_radians();
    let pitch = pitch.to_radians();
    let rx = x * yaw.cos() - y * yaw.sin();
    let ry = x * yaw.sin() + y * yaw.cos();
    let screen_y = ry * pitch.cos() - z * pitch.sin();
    let depth = ry * pitch.sin() + z * pitch.cos();
    let scale = if perspective {
        (1.0 / (1.0 + depth * 0.08)).clamp(0.55, 1.8)
    } else {
        1.0
    };
    egui::pos2(
        center.x + pan.x + rx * 20.0 * zoom * scale,
        center.y + pan.y + screen_y * 20.0 * zoom * scale,
    )
}
fn draw_face(p: &egui::Painter, points: Vec<egui::Pos2>, color: egui::Color32) {
    p.add(egui::Shape::convex_polygon(
        points,
        color,
        egui::Stroke::NONE,
    ));
}
#[allow(clippy::too_many_arguments)]
fn draw_voxel(
    p: &egui::Painter,
    center: egui::Pos2,
    pos: GridPosition,
    color: Color,
    yaw: f32,
    zoom: f32,
    pitch: f32,
    pan: egui::Vec2,
    perspective: bool,
    occupied: &BTreeSet<GridPosition>,
) {
    let x = pos.x as f32;
    let y = pos.y as f32;
    let z = pos.z as f32;
    let v = |dx, dy, dz| {
        project(
            center,
            x + dx,
            y + dy,
            z + dz,
            yaw,
            pitch,
            zoom,
            pan,
            perspective,
        )
    };
    let a = yaw.to_radians();
    let sx = if a.sin() >= 0.0 { 1 } else { -1 };
    let sy = if a.cos() >= 0.0 { 1 } else { -1 };
    let base = egui::Color32::from_rgb(color.0, color.1, color.2);
    let dark = egui::Color32::from_rgb(color.0 / 2, color.1 / 2, color.2 / 2);
    let light = egui::Color32::from_rgb(
        color.0.saturating_add(35),
        color.1.saturating_add(35),
        color.2.saturating_add(35),
    );
    if !occupied.contains(&GridPosition::new(pos.x, pos.y, pos.z + 1)) {
        draw_face(
            p,
            vec![
                v(-0.5, -0.5, 0.5),
                v(0.5, -0.5, 0.5),
                v(0.5, 0.5, 0.5),
                v(-0.5, 0.5, 0.5),
            ],
            light,
        );
    }
    if !occupied.contains(&GridPosition::new(pos.x + sx, pos.y, pos.z)) {
        let q = sx as f32 * 0.5;
        draw_face(
            p,
            vec![
                v(q, -0.5, -0.5),
                v(q, 0.5, -0.5),
                v(q, 0.5, 0.5),
                v(q, -0.5, 0.5),
            ],
            dark,
        );
    }
    if !occupied.contains(&GridPosition::new(pos.x, pos.y + sy, pos.z)) {
        let q = sy as f32 * 0.5;
        draw_face(
            p,
            vec![
                v(-0.5, q, -0.5),
                v(0.5, q, -0.5),
                v(0.5, q, 0.5),
                v(-0.5, q, 0.5),
            ],
            base,
        );
    }
}
pub fn run() -> eframe::Result {
    eframe::run_native(
        "ハコニワ",
        eframe::NativeOptions::default(),
        Box::new(|cc| Ok(Box::new(HakoniwaApp::new(&cc.egui_ctx)))),
    )
}
