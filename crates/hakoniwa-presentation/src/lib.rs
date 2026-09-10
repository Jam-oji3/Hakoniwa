use eframe::egui;
use hakoniwa_domain::{Bead, Color, GridPosition, Piece, Plane, Project};
use std::{collections::BTreeSet, sync::Arc};
pub struct HakoniwaApp {
    project: Project,
    selected: Option<u64>,
    status: String,
    yaw: f32,
    zoom: f32,
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
        Self {
            project: hammer_project(),
            selected: None,
            status: "ハンマーのMVPサンプルを読み込みました".into(),
            yaw: 45.0,
            zoom: 1.0,
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
        ui.horizontal(|ui| {
            if ui.button("ハンマーを新規作成").clicked() {
                self.project = hammer_project();
                self.selected = None;
                self.status = "ハンマーを作成しました".into();
            }
            ui.label(&self.status);
        });
        ui.separator();
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
    let (rect, _) = ui.allocate_exact_size(egui::vec2(330.0, 330.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, egui::Color32::from_gray(25));
    for i in 0..11 {
        let x = rect.left() + i as f32 * 30.0;
        let y = rect.top() + i as f32 * 30.0;
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            egui::Stroke::new(1.0, egui::Color32::DARK_GRAY),
        );
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            egui::Stroke::new(1.0, egui::Color32::DARK_GRAY),
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
fn draw_preview(ui: &mut egui::Ui, project: &Project, yaw: &mut f32, zoom: &mut f32) {
    ui.heading("3D Assembly ビュー");
    ui.horizontal(|ui| {
        ui.label("カメラ回転");
        ui.add(egui::Slider::new(yaw, 0.0..=360.0).suffix("°"));
    });
    ui.horizontal(|ui| {
        ui.label("ズーム");
        ui.add(egui::Slider::new(zoom, 0.5..=2.0));
    });
    let (rect, response) = ui.allocate_exact_size(egui::vec2(330.0, 330.0), egui::Sense::drag());
    if response.hovered() {
        ui.input(|input| {
            if input.pointer.button_down(egui::PointerButton::Middle) {
                *yaw = (*yaw + input.pointer.delta().x * 0.5).rem_euclid(360.0);
            }
            if input.smooth_scroll_delta.y != 0.0 {
                *zoom = (*zoom * (1.0 + input.smooth_scroll_delta.y * 0.001)).clamp(0.5, 2.0);
            }
        });
    }
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, egui::Color32::from_gray(18));
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
                &occupied,
            );
        }
    }
}
fn project(center: egui::Pos2, x: f32, y: f32, z: f32, yaw: f32, zoom: f32) -> egui::Pos2 {
    let a = yaw.to_radians();
    egui::pos2(
        center.x + (x * a.cos() - y * a.sin()) * 20.0 * zoom,
        center.y + (x * a.sin() + y * a.cos()) * 9.0 * zoom - z * 20.0 * zoom,
    )
}
fn draw_face(p: &egui::Painter, points: Vec<egui::Pos2>, color: egui::Color32) {
    p.add(egui::Shape::convex_polygon(
        points,
        color,
        egui::Stroke::NONE,
    ));
}
fn draw_voxel(
    p: &egui::Painter,
    center: egui::Pos2,
    pos: GridPosition,
    color: Color,
    yaw: f32,
    zoom: f32,
    occupied: &BTreeSet<GridPosition>,
) {
    let x = pos.x as f32;
    let y = pos.y as f32;
    let z = pos.z as f32;
    let v = |dx, dy, dz| project(center, x + dx, y + dy, z + dz, yaw, zoom);
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
