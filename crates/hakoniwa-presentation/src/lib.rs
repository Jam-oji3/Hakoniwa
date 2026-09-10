use eframe::egui;
use hakoniwa_domain::{Bead, Color, GridPosition, Piece, Plane, Project};
use std::sync::Arc;

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
        ui.label("回転");
        ui.add(egui::Slider::new(yaw, 0.0..=360.0).suffix("°"));
    });
    ui.horizontal(|ui| {
        ui.label("ズーム");
        ui.add(egui::Slider::new(zoom, 0.5..=2.0));
    });
    let (rect, _) = ui.allocate_exact_size(egui::vec2(330.0, 330.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, egui::Color32::from_gray(18));
    let rad = yaw.to_radians();
    for piece in project.pieces.values() {
        for (p, c) in &piece.beads {
            let rx = p.x as f32 * rad.cos() - p.y as f32 * rad.sin();
            let ry = p.x as f32 * rad.sin() + p.y as f32 * rad.cos();
            let center = egui::pos2(
                rect.center().x + rx * 20.0 * *zoom,
                rect.center().y + ry * 10.0 * *zoom - p.z as f32 * 18.0 * *zoom,
            );
            draw_cube(painter, center, 9.0 * *zoom, *c);
        }
    }
}
fn draw_cube(p: &egui::Painter, c: egui::Pos2, s: f32, col: Color) {
    let base = egui::Color32::from_rgb(col.0, col.1, col.2);
    let dark = egui::Color32::from_rgb(col.0 / 2, col.1 / 2, col.2 / 2);
    let light = egui::Color32::from_rgb(
        col.0.saturating_add(35),
        col.1.saturating_add(35),
        col.2.saturating_add(35),
    );
    let top = vec![
        egui::pos2(c.x, c.y - s),
        egui::pos2(c.x + s, c.y - s / 2.0),
        egui::pos2(c.x, c.y),
        egui::pos2(c.x - s, c.y - s / 2.0),
    ];
    let left = vec![
        top[3],
        top[2],
        egui::pos2(c.x, c.y + s),
        egui::pos2(c.x - s, c.y + s / 2.0),
    ];
    let right = vec![
        top[1],
        egui::pos2(c.x + s, c.y + s / 2.0),
        egui::pos2(c.x, c.y + s),
        top[2],
    ];
    p.add(egui::Shape::convex_polygon(left, dark, egui::Stroke::NONE));
    p.add(egui::Shape::convex_polygon(right, base, egui::Stroke::NONE));
    p.add(egui::Shape::convex_polygon(top, light, egui::Stroke::NONE));
}
pub fn run() -> eframe::Result {
    eframe::run_native(
        "ハコニワ",
        eframe::NativeOptions::default(),
        Box::new(|cc| Ok(Box::new(HakoniwaApp::new(&cc.egui_ctx)))),
    )
}
