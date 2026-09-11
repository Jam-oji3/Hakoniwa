mod editor_2d;

use eframe::egui;
use glam::{Quat, Vec3};
use hakoniwa_application::{Command, Editor};
use hakoniwa_domain::{
    Bead, Color, GridPosition, ObjectId, ObjectRef, Piece, Plane, Project, VoxelObjectRef,
};
use std::{collections::BTreeSet, sync::Arc};

use editor_2d::{GridPoint2d, PlateLayout, Viewport2d, project_position, unproject_position};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorkspacePane {
    PartsTree,
    PieceEditor,
    AssemblyView,
    Inspector,
}

#[derive(Clone, Copy)]
struct WorkspacePaneIds {
    parts_tree: egui_tiles::TileId,
    piece_editor: egui_tiles::TileId,
    assembly_view: egui_tiles::TileId,
    inspector: egui_tiles::TileId,
}

struct PendingCommand {
    command: Command,
    select_created: bool,
}
pub struct HakoniwaApp {
    editor: Editor,
    selected: Option<ObjectRef>,
    piece_editor: PieceEditorState,
    tree_editor: TreeEditorState,
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
            editor: Editor::new(hammer_project()),
            selected: None,
            piece_editor: PieceEditorState::default(),
            tree_editor: TreeEditorState::default(),
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
                self.editor = Editor::new(hammer_project());
                self.selected = None;
                self.piece_editor.reset();
                self.status = "ハンマーを作成しました".into();
            }
            let undo_requested = ui
                .add_enabled(self.editor.can_undo(), egui::Button::new("Undo"))
                .clicked()
                || ui.ctx().input_mut(|input| {
                    input.consume_shortcut(&egui::KeyboardShortcut::new(
                        egui::Modifiers::CTRL,
                        egui::Key::Z,
                    ))
                });
            let redo_requested = ui
                .add_enabled(self.editor.can_redo(), egui::Button::new("Redo"))
                .clicked()
                || ui.ctx().input_mut(|input| {
                    input.consume_shortcut(&egui::KeyboardShortcut::new(
                        egui::Modifiers::CTRL,
                        egui::Key::Y,
                    ))
                });
            if undo_requested && self.editor.undo() {
                self.status = "操作を元に戻しました".into();
            }
            if redo_requested && self.editor.redo() {
                self.status = "操作をやり直しました".into();
            }
            ui.label(&self.status);
            ui.separator();
            ui.menu_button("表示", |ui| {
                for (title, tile_id) in [
                    ("パーツツリー", self.pane_ids.parts_tree),
                    ("2Dエディタ", self.pane_ids.piece_editor),
                    ("3D View", self.pane_ids.assembly_view),
                    ("インスペクタ", self.pane_ids.inspector),
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

        let commands = {
            let mut behavior = WorkspaceBehavior {
                project: self.editor.project(),
                selected: &mut self.selected,
                piece_editor: &mut self.piece_editor,
                tree_editor: &mut self.tree_editor,
                zoom: &mut self.zoom,
                orientation: &mut self.orientation,
                pan: &mut self.pan,
                perspective: &mut self.perspective,
                close_requested: None,
                commands: Vec::new(),
            };
            self.workspace.ui(&mut behavior, ui);
            if let Some(tile_id) = behavior.close_requested {
                self.workspace.tiles.set_visible(tile_id, false);
            }
            behavior.commands
        };

        for pending in commands {
            match self.editor.execute(pending.command) {
                Ok(result) => {
                    if pending.select_created && result.created_object.is_some() {
                        self.selected = result.created_object;
                    }
                    self.status = "編集しました".into();
                }
                Err(error) => self.status = format!("編集できません: {error:?}"),
            }
        }
    }
}

fn create_workspace_tree() -> (egui_tiles::Tree<WorkspacePane>, WorkspacePaneIds) {
    let mut tiles = egui_tiles::Tiles::default();
    let pane_ids = WorkspacePaneIds {
        parts_tree: tiles.insert_pane(WorkspacePane::PartsTree),
        piece_editor: tiles.insert_pane(WorkspacePane::PieceEditor),
        assembly_view: tiles.insert_pane(WorkspacePane::AssemblyView),
        inspector: tiles.insert_pane(WorkspacePane::Inspector),
    };
    let parts_tabs = tiles.insert_tab_tile(vec![pane_ids.parts_tree]);
    let editor_tabs = tiles.insert_tab_tile(vec![pane_ids.piece_editor]);
    let assembly_tabs = tiles.insert_tab_tile(vec![pane_ids.assembly_view]);
    let inspector_tabs = tiles.insert_tab_tile(vec![pane_ids.inspector]);
    let root =
        tiles.insert_horizontal_tile(vec![parts_tabs, editor_tabs, assembly_tabs, inspector_tabs]);
    (
        egui_tiles::Tree::new("hakoniwa-workspace", root, tiles),
        pane_ids,
    )
}

struct WorkspaceBehavior<'a> {
    project: &'a Project,
    selected: &'a mut Option<ObjectRef>,
    piece_editor: &'a mut PieceEditorState,
    tree_editor: &'a mut TreeEditorState,
    zoom: &'a mut f32,
    orientation: &'a mut Quat,
    pan: &'a mut egui::Vec2,
    perspective: &'a mut bool,
    close_requested: Option<egui_tiles::TileId>,
    commands: Vec<PendingCommand>,
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
            WorkspacePane::PartsTree => draw_parts_tree(
                ui,
                self.project,
                self.selected,
                self.tree_editor,
                &mut self.commands,
            ),
            WorkspacePane::PieceEditor => draw_editor(
                ui,
                self.project
                    .pieces
                    .get(&selected_piece_id(*self.selected).unwrap_or_default()),
                self.piece_editor,
                &mut self.commands,
            ),
            WorkspacePane::AssemblyView => draw_preview(
                ui,
                self.project,
                self.zoom,
                self.orientation,
                self.pan,
                self.perspective,
            ),
            WorkspacePane::Inspector => draw_inspector(ui, self.project, *self.selected),
        }
        egui_tiles::UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &WorkspacePane) -> egui::WidgetText {
        match pane {
            WorkspacePane::PartsTree => "パーツツリー",
            WorkspacePane::PieceEditor => "2Dエディタ",
            WorkspacePane::AssemblyView => "3D View",
            WorkspacePane::Inspector => "インスペクタ",
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PieceTool {
    Pencil,
    Eraser,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum NewPiecePlane {
    #[default]
    Xy,
    Xz,
    Yz,
}

impl NewPiecePlane {
    const fn plane(self) -> Plane {
        match self {
            Self::Xy => Plane::Xy { z: 0 },
            Self::Xz => Plane::Xz { y: 0 },
            Self::Yz => Plane::Yz { x: 0 },
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Xy => "XY",
            Self::Xz => "XZ",
            Self::Yz => "YZ",
        }
    }
}

#[derive(Default)]
struct TreeEditorState {
    new_name: String,
    new_piece_plane: NewPiecePlane,
}

struct PieceEditorState {
    piece_id: Option<ObjectId>,
    layout: PlateLayout,
    viewport: Option<Viewport2d>,
    tool: PieceTool,
    color: Color,
    last_dragged_cell: Option<GridPoint2d>,
}

impl Default for PieceEditorState {
    fn default() -> Self {
        Self {
            piece_id: None,
            layout: PlateLayout::empty_piece(),
            viewport: None,
            tool: PieceTool::Pencil,
            color: Color::RED,
            last_dragged_cell: None,
        }
    }
}

impl PieceEditorState {
    fn reset(&mut self) {
        self.piece_id = None;
        self.layout = PlateLayout::empty_piece();
        self.viewport = None;
        self.last_dragged_cell = None;
    }

    fn ensure_piece(&mut self, piece: &Piece, size: egui::Vec2) {
        if self.piece_id == Some(piece.id) && self.viewport.is_some() {
            return;
        }
        self.piece_id = Some(piece.id);
        self.layout = PlateLayout::for_piece(piece);
        self.viewport = Some(Viewport2d::for_layout(
            self.layout,
            size.x.max(1.0),
            size.y.max(1.0),
        ));
        self.last_dragged_cell = None;
    }
}

fn selected_piece_id(selected: Option<ObjectRef>) -> Option<ObjectId> {
    match selected {
        Some(ObjectRef::Piece(id)) => Some(id),
        _ => None,
    }
}

fn draw_parts_tree(
    ui: &mut egui::Ui,
    project: &Project,
    selected: &mut Option<ObjectRef>,
    state: &mut TreeEditorState,
    commands: &mut Vec<PendingCommand>,
) {
    ui.horizontal(|ui| {
        ui.label("名前");
        ui.text_edit_singleline(&mut state.new_name);
    });
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("new-piece-plane")
            .selected_text(state.new_piece_plane.label())
            .show_ui(ui, |ui| {
                for plane in [NewPiecePlane::Xy, NewPiecePlane::Xz, NewPiecePlane::Yz] {
                    ui.selectable_value(&mut state.new_piece_plane, plane, plane.label());
                }
            });
        let parent_group_id = selected_parent_group(project, *selected);
        if ui.button("＋Piece").clicked() {
            let name = non_empty_name(&state.new_name, "Piece", project.pieces.len() + 1);
            commands.push(PendingCommand {
                command: Command::CreatePiece {
                    parent_group_id,
                    name,
                    plane: state.new_piece_plane.plane(),
                },
                select_created: true,
            });
            state.new_name.clear();
        }
        if ui.button("＋Group").clicked() {
            let name = non_empty_name(&state.new_name, "Group", project.groups.len());
            commands.push(PendingCommand {
                command: Command::CreateGroup {
                    parent_group_id,
                    name,
                },
                select_created: true,
            });
            state.new_name.clear();
        }
    });
    ui.separator();
    draw_group_tree(ui, project, project.root_group_id(), selected, commands);
    ui.separator();
    ui.label(format!("総ビーズ数: {}", project.inventory().total));
}

fn non_empty_name(input: &str, prefix: &str, number: usize) -> String {
    let input = input.trim();
    if input.is_empty() {
        format!("{prefix} {number}")
    } else {
        input.to_owned()
    }
}

fn selected_parent_group(project: &Project, selected: Option<ObjectRef>) -> ObjectId {
    match selected {
        Some(ObjectRef::Group(id)) if project.groups.contains_key(&id) => id,
        Some(ObjectRef::Piece(id)) => project
            .pieces
            .get(&id)
            .map_or(project.root_group_id(), |piece| piece.parent_group_id),
        Some(ObjectRef::Shape(id)) => project
            .shapes
            .get(&id)
            .map_or(project.root_group_id(), |shape| shape.parent_group_id),
        _ => project.root_group_id(),
    }
}

fn draw_group_tree(
    ui: &mut egui::Ui,
    project: &Project,
    group_id: ObjectId,
    selected: &mut Option<ObjectRef>,
    commands: &mut Vec<PendingCommand>,
) {
    let Some(group) = project.groups.get(&group_id) else {
        return;
    };
    draw_object_row(
        ui,
        format!("▾ {}", group.name),
        ObjectRef::Group(group.id),
        group.visible,
        selected,
        commands,
    );
    ui.indent(("group-children", group.id), |ui| {
        for child in project
            .groups
            .values()
            .filter(|child| child.parent_group_id == Some(group.id))
        {
            draw_group_tree(ui, project, child.id, selected, commands);
        }
        for shape in project
            .shapes
            .values()
            .filter(|shape| shape.parent_group_id == group.id)
        {
            draw_object_row(
                ui,
                format!("◆ {} ({} beads)", shape.name, shape.beads.len()),
                ObjectRef::Shape(shape.id),
                shape.visible,
                selected,
                commands,
            );
        }
        for piece in project
            .pieces
            .values()
            .filter(|piece| piece.parent_group_id == group.id)
        {
            draw_object_row(
                ui,
                format!("▦ {} ({} beads)", piece.name, piece.beads.len()),
                ObjectRef::Piece(piece.id),
                piece.visible,
                selected,
                commands,
            );
        }
    });
}

fn draw_object_row(
    ui: &mut egui::Ui,
    label: String,
    object: ObjectRef,
    visible: bool,
    selected: &mut Option<ObjectRef>,
    commands: &mut Vec<PendingCommand>,
) {
    ui.horizontal(|ui| {
        let mut requested_visibility = visible;
        if ui.checkbox(&mut requested_visibility, "").changed() {
            commands.push(PendingCommand {
                command: Command::SetVisibility {
                    object,
                    visible: requested_visibility,
                },
                select_created: false,
            });
        }
        if ui
            .selectable_label(*selected == Some(object), label)
            .clicked()
        {
            *selected = Some(object);
        }
    });
}
fn draw_editor(
    ui: &mut egui::Ui,
    piece: Option<&Piece>,
    state: &mut PieceEditorState,
    commands: &mut Vec<PendingCommand>,
) {
    let Some(piece) = piece else {
        state.reset();
        ui.label("ツリーからPieceを選択してください");
        return;
    };

    state.ensure_piece(piece, ui.available_size());
    ui.horizontal_wrapped(|ui| {
        ui.label(format!("{} · {:?}", piece.name, piece.plane));
        ui.separator();
        ui.selectable_value(&mut state.tool, PieceTool::Pencil, "ペン");
        ui.selectable_value(&mut state.tool, PieceTool::Eraser, "消しゴム");
        ui.separator();
        for (name, color) in editor_colors() {
            let fill = egui::Color32::from_rgb(color.0, color.1, color.2);
            if ui
                .add(
                    egui::Button::new(name)
                        .fill(fill)
                        .selected(state.color == color),
                )
                .clicked()
            {
                state.color = color;
                state.tool = PieceTool::Pencil;
            }
        }
        ui.separator();
        ui.label(format!(
            "{}×{} plates · {}×{} cells",
            state.layout.columns,
            state.layout.rows,
            state.layout.columns * 29,
            state.layout.rows * 29
        ));
    });
    ui.label("左ドラッグ: 編集 / 中ホイールドラッグ: 移動 / ホイール: ズーム");

    let available = ui.available_size();
    let (rect, response) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());
    let Some(viewport) = state.viewport.as_mut() else {
        return;
    };
    let screen_center = (rect.center().x, rect.center().y);

    if response.hovered() {
        ui.input(|input| {
            if input.pointer.button_down(egui::PointerButton::Middle) {
                let delta = input.pointer.delta();
                viewport.pan_pixels(delta.x, delta.y);
            }
            if input.smooth_scroll_delta.y != 0.0
                && let Some(pointer) = input.pointer.hover_pos()
            {
                let factor = (input.smooth_scroll_delta.y * 0.0015).exp();
                viewport.zoom_at(factor, (pointer.x, pointer.y), screen_center);
            }
        });
    }

    let painter = ui.painter().with_clip_rect(rect);
    draw_piece_grid(&painter, rect, state.layout, *viewport);
    draw_piece_beads(&painter, rect, piece, *viewport);

    let pointer_cell = response
        .hover_pos()
        .map(|pointer| viewport.screen_to_cell((pointer.x, pointer.y), screen_center));
    if let Some(cell) = pointer_cell {
        draw_cell_highlight(&painter, rect, cell, *viewport);
    }

    let primary_down = ui.input(|input| input.pointer.primary_down());
    if !primary_down {
        state.last_dragged_cell = None;
    } else if response.hovered()
        && pointer_cell.is_some()
        && pointer_cell != state.last_dragged_cell
    {
        let cell = pointer_cell.expect("pointer cell was checked");
        state.last_dragged_cell = Some(cell);
        let position = unproject_position(piece.plane, cell);
        let existing = piece.beads.get(&position).copied();
        let target = VoxelObjectRef::Piece(piece.id);
        match (state.tool, existing) {
            (PieceTool::Pencil, None) => {
                state.layout.expand_to_include(cell);
                commands.push(PendingCommand {
                    command: Command::AddBead {
                        target,
                        bead: Bead {
                            position,
                            color: state.color,
                        },
                    },
                    select_created: false,
                });
            }
            (PieceTool::Pencil, Some(color)) if color != state.color => {
                commands.push(PendingCommand {
                    command: Command::RecolorBead {
                        target,
                        position,
                        color: state.color,
                    },
                    select_created: false,
                });
            }
            (PieceTool::Eraser, Some(_)) => {
                commands.push(PendingCommand {
                    command: Command::RemoveBead { target, position },
                    select_created: false,
                });
            }
            _ => {}
        }
    }
}

fn editor_colors() -> [(&'static str, Color); 7] {
    [
        ("赤", Color::RED),
        ("茶", Color::BROWN),
        ("黄", Color(242, 196, 56)),
        ("緑", Color(60, 160, 90)),
        ("青", Color(55, 110, 210)),
        ("黒", Color(35, 35, 35)),
        ("白", Color(245, 245, 245)),
    ]
}

fn draw_piece_grid(
    painter: &egui::Painter,
    rect: egui::Rect,
    layout: PlateLayout,
    viewport: Viewport2d,
) {
    painter.rect_filled(rect, 0.0, egui::Color32::from_gray(250));
    let center = (rect.center().x, rect.center().y);
    let top_left = viewport.screen_to_grid((rect.left(), rect.top()), center);
    let bottom_right = viewport.screen_to_grid((rect.right(), rect.bottom()), center);
    let visible_min_u = top_left.0.min(bottom_right.0).floor() as i32 - 1;
    let visible_max_u = top_left.0.max(bottom_right.0).ceil() as i32 + 1;
    let visible_min_v = top_left.1.min(bottom_right.1).floor() as i32 - 1;
    let visible_max_v = top_left.1.max(bottom_right.1).ceil() as i32 + 1;

    let plate_min = viewport.grid_to_screen(
        (layout.min_u as f32 - 0.5, layout.max_v() as f32 + 0.5),
        center,
    );
    let plate_max = viewport.grid_to_screen(
        (layout.max_u() as f32 + 0.5, layout.min_v as f32 - 0.5),
        center,
    );
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(plate_min.0, plate_min.1),
            egui::pos2(plate_max.0, plate_max.1),
        ),
        0.0,
        egui::Color32::WHITE,
    );

    let auxiliary_stroke = egui::Stroke::new(0.5, egui::Color32::from_gray(232));
    if viewport.pixels_per_cell >= 5.0 {
        for u in visible_min_u..=visible_max_u {
            let x = viewport.grid_to_screen((u as f32 - 0.5, 0.0), center).0;
            painter.line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                auxiliary_stroke,
            );
        }
        for v in visible_min_v..=visible_max_v {
            let y = viewport.grid_to_screen((0.0, v as f32 - 0.5), center).1;
            painter.line_segment(
                [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                auxiliary_stroke,
            );
        }
    }

    let cell_stroke = egui::Stroke::new(1.0, egui::Color32::from_gray(195));
    let active_min_u = layout.min_u.max(visible_min_u);
    let active_max_u = layout.max_u().min(visible_max_u);
    let active_min_v = layout.min_v.max(visible_min_v);
    let active_max_v = layout.max_v().min(visible_max_v);
    if active_min_u <= active_max_u {
        for u in active_min_u..=active_max_u.saturating_add(1) {
            let x = viewport.grid_to_screen((u as f32 - 0.5, 0.0), center).0;
            painter.line_segment(
                [egui::pos2(x, plate_min.1), egui::pos2(x, plate_max.1)],
                cell_stroke,
            );
        }
    }
    if active_min_v <= active_max_v {
        for v in active_min_v..=active_max_v.saturating_add(1) {
            let y = viewport.grid_to_screen((0.0, v as f32 - 0.5), center).1;
            painter.line_segment(
                [egui::pos2(plate_min.0, y), egui::pos2(plate_max.0, y)],
                cell_stroke,
            );
        }
    }

    let plate_stroke = egui::Stroke::new(2.0, egui::Color32::from_rgb(80, 115, 160));
    for column in 0..=layout.columns {
        let u = layout.min_u as f32 + column as f32 * 29.0 - 0.5;
        let x = viewport.grid_to_screen((u, 0.0), center).0;
        painter.line_segment(
            [egui::pos2(x, plate_min.1), egui::pos2(x, plate_max.1)],
            plate_stroke,
        );
    }
    for row in 0..=layout.rows {
        let v = layout.min_v as f32 + row as f32 * 29.0 - 0.5;
        let y = viewport.grid_to_screen((0.0, v), center).1;
        painter.line_segment(
            [egui::pos2(plate_min.0, y), egui::pos2(plate_max.0, y)],
            plate_stroke,
        );
    }
}

fn draw_piece_beads(
    painter: &egui::Painter,
    rect: egui::Rect,
    piece: &Piece,
    viewport: Viewport2d,
) {
    let center = (rect.center().x, rect.center().y);
    for (position, color) in &piece.beads {
        let point = project_position(piece.plane, *position);
        let screen = viewport.grid_to_screen((point.u as f32, point.v as f32), center);
        painter.circle_filled(
            egui::pos2(screen.0, screen.1),
            (viewport.pixels_per_cell * 0.38).max(1.0),
            egui::Color32::from_rgb(color.0, color.1, color.2),
        );
    }
}

fn draw_cell_highlight(
    painter: &egui::Painter,
    rect: egui::Rect,
    cell: GridPoint2d,
    viewport: Viewport2d,
) {
    let center = (rect.center().x, rect.center().y);
    let min = viewport.grid_to_screen((cell.u as f32 - 0.5, cell.v as f32 + 0.5), center);
    let max = viewport.grid_to_screen((cell.u as f32 + 0.5, cell.v as f32 - 0.5), center);
    painter.rect_stroke(
        egui::Rect::from_min_max(egui::pos2(min.0, min.1), egui::pos2(max.0, max.1)),
        0.0,
        egui::Stroke::new(2.0, egui::Color32::from_rgb(40, 105, 210)),
        egui::StrokeKind::Inside,
    );
}

fn draw_inspector(ui: &mut egui::Ui, project: &Project, selected: Option<ObjectRef>) {
    let Some(selected) = selected else {
        ui.label("ツリーから対象を選択してください");
        return;
    };
    match selected {
        ObjectRef::Group(id) => {
            let Some(group) = project.groups.get(&id) else {
                ui.label("選択したGroupは存在しません");
                return;
            };
            ui.heading(&group.name);
            ui.label("種類: Group");
            ui.label(format!("ID: {}", group.id));
            ui.label(format!("表示: {}", visibility_label(group.visible)));
            ui.label(format!("配置: {:?}", group.placement.translation));
        }
        ObjectRef::Shape(id) => {
            let Some(shape) = project.shapes.get(&id) else {
                ui.label("選択したShapeは存在しません");
                return;
            };
            ui.heading(&shape.name);
            ui.label("種類: Shape");
            ui.label(format!("ID: {}", shape.id));
            ui.label(format!("ビーズ数: {}", shape.beads.len()));
            ui.label(format!("表示: {}", visibility_label(shape.visible)));
        }
        ObjectRef::Piece(id) => {
            let Some(piece) = project.pieces.get(&id) else {
                ui.label("選択したPieceは存在しません");
                return;
            };
            let layout = PlateLayout::for_piece(piece);
            ui.heading(&piece.name);
            ui.label("種類: Piece");
            ui.label(format!("ID: {}", piece.id));
            ui.label(format!("平面: {}", plane_label(piece.plane)));
            ui.label(format!("ビーズ数: {}", piece.beads.len()));
            ui.label(format!(
                "最小プレート: {}×{}（{}枚）",
                layout.columns,
                layout.rows,
                layout.columns * layout.rows
            ));
            ui.label(format!("表示: {}", visibility_label(piece.visible)));
        }
    }
}

const fn visibility_label(visible: bool) -> &'static str {
    if visible { "表示" } else { "非表示" }
}

const fn plane_label(plane: Plane) -> &'static str {
    match plane {
        Plane::Xy { .. } => "XY",
        Plane::Xz { .. } => "XZ",
        Plane::Yz { .. } => "YZ",
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
