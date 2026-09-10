use eframe::egui;
use glam::{Quat, Vec3};
use hakoniwa_domain::{Bead, Color, GridPosition, Piece, Plane, Project};
use std::{collections::BTreeSet, sync::Arc};
pub struct HakoniwaApp {
    project: Project,
    selected: Option<u64>,
    status: String,
    zoom: f32,
    azimuth: f32,
    elevation: f32,
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
            zoom: 1.0,
            azimuth: 45.0,
            elevation: 35.0,
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
                &mut self.zoom,
                &mut self.azimuth,
                &mut self.elevation,
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
    zoom: &mut f32,
    azimuth: &mut f32,
    elevation: &mut f32,
    pan: &mut egui::Vec2,
    perspective: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.heading("3D Assembly ビュー");
        ui.checkbox(perspective, "透視投影");
        if ui.button("ビューをリセット").clicked() {
            *azimuth = 45.0;
            *elevation = 35.0;
            *zoom = 1.0;
            *pan = egui::Vec2::ZERO;
        }
    });
    ui.label("中ホイールドラッグ: Turntable回転 / Shift+中ホイール: 移動 / ホイール: ズーム");
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::drag());
    if response.hovered() {
        ui.input(|input| {
            if input.pointer.button_down(egui::PointerButton::Middle) {
                let delta = input.pointer.delta();
                if input.modifiers.shift {
                    *pan += delta;
                } else {
                    *azimuth = (*azimuth + delta.x * 0.5).rem_euclid(360.0);
                    *elevation = (*elevation + delta.y * 0.5).rem_euclid(360.0);
                }
            }
            if input.smooth_scroll_delta.y != 0.0 {
                *zoom = (*zoom * (1.0 + input.smooth_scroll_delta.y * 0.001)).clamp(0.1, 8.0);
            }
        });
    }
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, egui::Color32::WHITE);
    render_voxels(
        &painter,
        rect,
        project,
        turntable_orientation(*azimuth, *elevation),
        *zoom,
        *pan,
        *perspective,
    );
}

fn turntable_orientation(azimuth: f32, elevation: f32) -> Quat {
    Quat::from_rotation_x(-elevation.to_radians()) * Quat::from_rotation_z(azimuth.to_radians())
}

struct Camera {
    orientation: Quat,
    target: Vec3,
    pixels_per_unit: f32,
    pan: egui::Vec2,
    perspective: bool,
    focal_distance: f32,
}

struct RenderFace {
    depth: f32,
    points: Vec<egui::Pos2>,
    color: egui::Color32,
}

fn render_voxels(
    painter: &egui::Painter,
    rect: egui::Rect,
    project: &Project,
    orientation: Quat,
    zoom: f32,
    pan: egui::Vec2,
    perspective: bool,
) {
    let occupied = project
        .pieces
        .values()
        .flat_map(|piece| piece.beads.keys().copied())
        .collect::<BTreeSet<_>>();
    if occupied.is_empty() {
        return;
    }
    let min = Vec3::new(
        occupied.iter().map(|p| p.x).min().unwrap() as f32,
        occupied.iter().map(|p| p.y).min().unwrap() as f32,
        occupied.iter().map(|p| p.z).min().unwrap() as f32,
    );
    let max = Vec3::new(
        occupied.iter().map(|p| p.x).max().unwrap() as f32,
        occupied.iter().map(|p| p.y).max().unwrap() as f32,
        occupied.iter().map(|p| p.z).max().unwrap() as f32,
    );
    let span = (max - min + Vec3::ONE).max_element().max(1.0);
    let camera = Camera {
        orientation,
        target: (min + max) * 0.5,
        pixels_per_unit: rect.width().min(rect.height()) * 0.65 / span * zoom,
        pan,
        perspective,
        focal_distance: span * 4.0,
    };
    let mut faces = Vec::new();
    for piece in project.pieces.values() {
        for (position, color) in &piece.beads {
            append_faces(
                &mut faces,
                rect.center(),
                *position,
                *color,
                &occupied,
                &camera,
            );
        }
    }
    faces.sort_by(|a, b| a.depth.total_cmp(&b.depth));
    for face in faces {
        painter.add(egui::Shape::convex_polygon(
            face.points,
            face.color,
            egui::Stroke::NONE,
        ));
    }
}

fn append_faces(
    output: &mut Vec<RenderFace>,
    center: egui::Pos2,
    position: GridPosition,
    color: Color,
    occupied: &BTreeSet<GridPosition>,
    camera: &Camera,
) {
    let x = position.x as f32;
    let y = position.y as f32;
    let z = position.z as f32;
    let definitions = [
        (
            GridPosition::new(1, 0, 0),
            Vec3::X,
            [
                Vec3::new(0.5, -0.5, -0.5),
                Vec3::new(0.5, 0.5, -0.5),
                Vec3::new(0.5, 0.5, 0.5),
                Vec3::new(0.5, -0.5, 0.5),
            ],
        ),
        (
            GridPosition::new(-1, 0, 0),
            Vec3::NEG_X,
            [
                Vec3::new(-0.5, 0.5, -0.5),
                Vec3::new(-0.5, -0.5, -0.5),
                Vec3::new(-0.5, -0.5, 0.5),
                Vec3::new(-0.5, 0.5, 0.5),
            ],
        ),
        (
            GridPosition::new(0, 1, 0),
            Vec3::Y,
            [
                Vec3::new(0.5, 0.5, -0.5),
                Vec3::new(-0.5, 0.5, -0.5),
                Vec3::new(-0.5, 0.5, 0.5),
                Vec3::new(0.5, 0.5, 0.5),
            ],
        ),
        (
            GridPosition::new(0, -1, 0),
            Vec3::NEG_Y,
            [
                Vec3::new(-0.5, -0.5, -0.5),
                Vec3::new(0.5, -0.5, -0.5),
                Vec3::new(0.5, -0.5, 0.5),
                Vec3::new(-0.5, -0.5, 0.5),
            ],
        ),
        (
            GridPosition::new(0, 0, 1),
            Vec3::Z,
            [
                Vec3::new(-0.5, -0.5, 0.5),
                Vec3::new(0.5, -0.5, 0.5),
                Vec3::new(0.5, 0.5, 0.5),
                Vec3::new(-0.5, 0.5, 0.5),
            ],
        ),
        (
            GridPosition::new(0, 0, -1),
            Vec3::NEG_Z,
            [
                Vec3::new(-0.5, 0.5, -0.5),
                Vec3::new(0.5, 0.5, -0.5),
                Vec3::new(0.5, -0.5, -0.5),
                Vec3::new(-0.5, -0.5, -0.5),
            ],
        ),
    ];
    for (neighbor, normal, corners) in definitions {
        let adjacent = GridPosition::new(
            position.x + neighbor.x,
            position.y + neighbor.y,
            position.z + neighbor.z,
        );
        if occupied.contains(&adjacent) {
            continue;
        }
        let camera_normal = camera.orientation * normal;
        if camera_normal.z <= 0.001 {
            continue;
        }
        let mut points = Vec::with_capacity(4);
        let mut depth = 0.0;
        for corner in corners {
            let camera_point = camera.orientation * (Vec3::new(x, y, z) + corner - camera.target);
            depth += camera_point.z;
            points.push(project_point(center, camera_point, camera));
        }
        let light = (0.45
            + 0.55
                * camera_normal
                    .dot(Vec3::new(0.3, 0.4, 0.866).normalize())
                    .abs())
        .clamp(0.35, 1.0);
        output.push(RenderFace {
            depth: depth / 4.0,
            points,
            color: egui::Color32::from_rgb(
                (color.0 as f32 * light) as u8,
                (color.1 as f32 * light) as u8,
                (color.2 as f32 * light) as u8,
            ),
        });
    }
}

fn project_point(center: egui::Pos2, point: Vec3, camera: &Camera) -> egui::Pos2 {
    let perspective_scale = if camera.perspective {
        (camera.focal_distance / (camera.focal_distance - point.z)).clamp(0.2, 5.0)
    } else {
        1.0
    };
    egui::pos2(
        center.x + camera.pan.x + point.x * camera.pixels_per_unit * perspective_scale,
        center.y + camera.pan.y - point.y * camera.pixels_per_unit * perspective_scale,
    )
}
pub fn run() -> eframe::Result {
    eframe::run_native(
        "ハコニワ",
        eframe::NativeOptions::default(),
        Box::new(|cc| Ok(Box::new(HakoniwaApp::new(&cc.egui_ctx)))),
    )
}
