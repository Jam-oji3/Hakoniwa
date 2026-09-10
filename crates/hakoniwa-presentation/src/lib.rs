use eframe::egui;
use glam::{Quat, Vec3};
use hakoniwa_domain::{Bead, Color, GridPosition, Piece, Plane, Project};
use std::{collections::BTreeSet, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorkspacePane {
    PartsTree,
    PieceEditor,
    AssemblyView,
}

#[derive(Clone, Copy)]
struct WorkspacePaneIds {
    parts_tree: egui_tiles::TileId,
    piece_editor: egui_tiles::TileId,
    assembly_view: egui_tiles::TileId,
}
pub struct HakoniwaApp {
    project: Project,
    selected: Option<u64>,
    status: String,
    zoom: f32,
    orientation: Quat,
    pan: egui::Vec2,
    perspective: bool,
    workspace: egui_tiles::Tree<WorkspacePane>,
    pane_ids: WorkspacePaneIds,
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
        let (workspace, pane_ids) = create_workspace_tree();
        Self {
            project: hammer_project(),
            selected: None,
            status: "ハンマーのMVPサンプルを読み込みました".into(),
            zoom: 1.0,
            orientation: Quat::from_rotation_x(-35.0_f32.to_radians())
                * Quat::from_rotation_z(45.0_f32.to_radians()),
            pan: egui::Vec2::ZERO,
            perspective: true,
            workspace,
            pane_ids,
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
            ui.separator();
            ui.menu_button("表示", |ui| {
                for (title, tile_id) in [
                    ("パーツツリー", self.pane_ids.parts_tree),
                    ("2Dエディタ", self.pane_ids.piece_editor),
                    ("3D View", self.pane_ids.assembly_view),
                ] {
                    let visible = self.workspace.tiles.is_visible(tile_id);
                    if ui.add_enabled(!visible, egui::Button::new(title)).clicked() {
                        self.workspace.tiles.set_visible(tile_id, true);
                        self.workspace.make_active(|id, _| id == tile_id);
                        ui.close();
                    }
                }
            });
            if ui.button("レイアウトをリセット").clicked() {
                let (workspace, pane_ids) = create_workspace_tree();
                self.workspace = workspace;
                self.pane_ids = pane_ids;
            }
        });
        ui.separator();

        let Self {
            project,
            selected,
            zoom,
            orientation,
            pan,
            perspective,
            workspace,
            ..
        } = self;
        let mut behavior = WorkspaceBehavior {
            project,
            selected,
            zoom,
            orientation,
            pan,
            perspective,
            close_requested: None,
        };
        workspace.ui(&mut behavior, ui);
        if let Some(tile_id) = behavior.close_requested {
            workspace.tiles.set_visible(tile_id, false);
        }
    }
}

fn create_workspace_tree() -> (egui_tiles::Tree<WorkspacePane>, WorkspacePaneIds) {
    let mut tiles = egui_tiles::Tiles::default();
    let pane_ids = WorkspacePaneIds {
        parts_tree: tiles.insert_pane(WorkspacePane::PartsTree),
        piece_editor: tiles.insert_pane(WorkspacePane::PieceEditor),
        assembly_view: tiles.insert_pane(WorkspacePane::AssemblyView),
    };
    let parts_tabs = tiles.insert_tab_tile(vec![pane_ids.parts_tree]);
    let editor_tabs = tiles.insert_tab_tile(vec![pane_ids.piece_editor]);
    let assembly_tabs = tiles.insert_tab_tile(vec![pane_ids.assembly_view]);
    let root = tiles.insert_horizontal_tile(vec![parts_tabs, editor_tabs, assembly_tabs]);
    (
        egui_tiles::Tree::new("hakoniwa-workspace", root, tiles),
        pane_ids,
    )
}

struct WorkspaceBehavior<'a> {
    project: &'a Project,
    selected: &'a mut Option<u64>,
    zoom: &'a mut f32,
    orientation: &'a mut Quat,
    pan: &'a mut egui::Vec2,
    perspective: &'a mut bool,
    close_requested: Option<egui_tiles::TileId>,
}

impl egui_tiles::Behavior<WorkspacePane> for WorkspaceBehavior<'_> {
    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        pane: &mut WorkspacePane,
    ) -> egui_tiles::UiResponse {
        ui.painter()
            .rect_filled(ui.max_rect(), 0.0, egui::Color32::WHITE);
        match pane {
            WorkspacePane::PartsTree => draw_parts_tree(ui, self.project, self.selected),
            WorkspacePane::PieceEditor => draw_editor(
                ui,
                self.project
                    .pieces
                    .get(&(*self.selected).unwrap_or_default()),
            ),
            WorkspacePane::AssemblyView => draw_preview(
                ui,
                self.project,
                self.zoom,
                self.orientation,
                self.pan,
                self.perspective,
            ),
        }
        egui_tiles::UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &WorkspacePane) -> egui::WidgetText {
        match pane {
            WorkspacePane::PartsTree => "パーツツリー",
            WorkspacePane::PieceEditor => "2Dエディタ",
            WorkspacePane::AssemblyView => "3D View",
        }
        .into()
    }

    fn is_tab_closable(
        &self,
        _tiles: &egui_tiles::Tiles<WorkspacePane>,
        _tile_id: egui_tiles::TileId,
    ) -> bool {
        true
    }

    fn on_tab_close(
        &mut self,
        _tiles: &mut egui_tiles::Tiles<WorkspacePane>,
        tile_id: egui_tiles::TileId,
    ) -> bool {
        self.close_requested = Some(tile_id);
        false
    }

    fn simplification_options(&self) -> egui_tiles::SimplificationOptions {
        egui_tiles::SimplificationOptions {
            all_panes_must_have_tabs: true,
            ..Default::default()
        }
    }

    fn min_size(&self) -> f32 {
        120.0
    }

    fn gap_width(&self, _style: &egui::Style) -> f32 {
        1.0
    }

    fn resize_stroke(
        &self,
        _style: &egui::Style,
        resize_state: egui_tiles::ResizeState,
    ) -> egui::Stroke {
        match resize_state {
            egui_tiles::ResizeState::Idle => egui::Stroke::new(1.0, egui::Color32::from_gray(175)),
            egui_tiles::ResizeState::Hovering => {
                egui::Stroke::new(2.0, egui::Color32::from_rgb(90, 140, 210))
            }
            egui_tiles::ResizeState::Dragging => {
                egui::Stroke::new(2.0, egui::Color32::from_rgb(55, 110, 190))
            }
        }
    }

    fn tab_bar_color(&self, _visuals: &egui::Visuals) -> egui::Color32 {
        egui::Color32::from_gray(242)
    }

    fn tab_bar_hline_stroke(&self, _visuals: &egui::Visuals) -> egui::Stroke {
        egui::Stroke::new(1.0, egui::Color32::from_gray(175))
    }
}

fn draw_parts_tree(ui: &mut egui::Ui, project: &Project, selected: &mut Option<u64>) {
    for (id, piece) in &project.pieces {
        if ui
            .selectable_label(
                *selected == Some(*id),
                format!("▦ {} ({} beads)", piece.name, piece.beads.len()),
            )
            .clicked()
        {
            *selected = Some(*id);
        }
    }
    ui.separator();
    ui.label(format!("総ビーズ数: {}", project.inventory().total));
}
fn draw_editor(ui: &mut egui::Ui, piece: Option<&Piece>) {
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
    orientation: &mut Quat,
    pan: &mut egui::Vec2,
    perspective: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.checkbox(perspective, "透視投影");
        if ui.button("ビューをリセット").clicked() {
            *orientation = Quat::from_rotation_x(-35.0_f32.to_radians())
                * Quat::from_rotation_z(45.0_f32.to_radians());
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
                    apply_turntable_drag(orientation, delta);
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
        *orientation,
        *zoom,
        *pan,
        *perspective,
    );
}

fn apply_turntable_drag(orientation: &mut Quat, delta: egui::Vec2) {
    const RADIANS_PER_PIXEL: f32 = std::f32::consts::PI / 360.0;

    let view_inverse = orientation.conjugate();
    let view_right = view_inverse * Vec3::X;
    let view_up = view_inverse * Vec3::Y;
    let view_direction = view_inverse * Vec3::Z;

    let mut pitch_axis = Vec3::Z.cross(view_direction);
    if pitch_axis.length_squared() > 0.001 {
        pitch_axis = pitch_axis.normalize();
        if pitch_axis.dot(view_right) < 0.0 {
            pitch_axis = -pitch_axis;
        }
        let mut blend =
            (Vec3::Z.angle_between(view_direction) / std::f32::consts::PI - 0.5).abs() * 2.0;
        blend *= blend;
        pitch_axis = pitch_axis.lerp(view_right, blend).normalize();
    } else {
        pitch_axis = view_right;
    }

    let yaw_direction = if view_up.z < 0.0 { -1.0 } else { 1.0 };
    let pitch = Quat::from_axis_angle(pitch_axis, delta.y * RADIANS_PER_PIXEL);
    let yaw = Quat::from_rotation_z(delta.x * RADIANS_PER_PIXEL * yaw_direction);
    *orientation = (*orientation * pitch * yaw).normalize();
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
