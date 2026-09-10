use eframe::egui;
use hakoniwa_domain::{Bead, Color, GridPosition, Piece, Plane, Project};

pub struct HakoniwaApp {
    project: Project,
    selected: Option<u64>,
    status: String,
}

impl Default for HakoniwaApp {
    fn default() -> Self {
        Self {
            project: hammer_project(),
            selected: None,
            status: "ハンマーのMVPサンプルを読み込みました".into(),
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
            columns[0].label(format!("総ビーズ数: {}", self.project.bead_count()));
            draw_editor(
                &mut columns[1],
                self.project.pieces.get(&self.selected.unwrap_or(0)),
            );
            draw_preview(&mut columns[2], &self.project);
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
    for x in 0..11 {
        let p = rect.left() + x as f32 * 30.0;
        painter.line_segment(
            [egui::pos2(p, rect.top()), egui::pos2(p, rect.bottom())],
            egui::Stroke::new(1.0, egui::Color32::DARK_GRAY),
        );
    }
    for y in 0..11 {
        let p = rect.top() + y as f32 * 30.0;
        painter.line_segment(
            [egui::pos2(rect.left(), p), egui::pos2(rect.right(), p)],
            egui::Stroke::new(1.0, egui::Color32::DARK_GRAY),
        );
    }
    for (pos, color) in &piece.beads {
        let px = rect.center().x + pos.x as f32 * 30.0;
        let py = rect.center().y - pos.y as f32 * 30.0;
        painter.circle_filled(
            egui::pos2(px, py),
            11.0,
            egui::Color32::from_rgb(color.0, color.1, color.2),
        );
    }
}
fn draw_preview(ui: &mut egui::Ui, project: &Project) {
    ui.heading("3D Assembly プレビュー");
    ui.label("直交面の簡易アイソメトリック表示");
    let (rect, _) = ui.allocate_exact_size(egui::vec2(330.0, 330.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, egui::Color32::from_gray(18));
    for piece in project.pieces.values() {
        for (p, c) in &piece.beads {
            let x = rect.center().x + (p.x - p.y) as f32 * 16.0;
            let y = rect.center().y + (p.x + p.y) as f32 * 8.0 - p.z as f32 * 16.0;
            painter.circle_filled(
                egui::pos2(x, y),
                7.0,
                egui::Color32::from_rgb(c.0, c.1, c.2),
            );
        }
    }
}

pub fn run() -> eframe::Result {
    eframe::run_native(
        "ハコニワ",
        eframe::NativeOptions::default(),
        Box::new(|_| Ok(Box::new(HakoniwaApp::default()))),
    )
}
