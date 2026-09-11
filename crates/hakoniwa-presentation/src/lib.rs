mod editor_2d;

use eframe::egui;
use glam::{Quat, Vec3};
use hakoniwa_application::{Command, Editor, ProjectRepository};
use hakoniwa_domain::{
    Bead, Color, GridAxis, GridPosition, GridRotation, ObjectId, ObjectRef, Piece, Placement,
    Plane, Project, VoxelObjectRef,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Debug,
    path::PathBuf,
    sync::Arc,
};

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
pub struct HakoniwaApp<R: ProjectRepository> {
    editor: Editor,
    repository: R,
    file_path: String,
    selected: Option<ObjectRef>,
    piece_editor: PieceEditorState,
    tree_editor: TreeEditorState,
    assembly_editor: AssemblyEditorState,
    status: String,
    zoom: f32,
    orientation: Quat,
    pan: egui::Vec2,
    perspective: bool,
    workspace: egui_tiles::Tree<WorkspacePane>,
    pane_ids: WorkspacePaneIds,
}
impl<R: ProjectRepository> HakoniwaApp<R> {
    fn new(ctx: &egui::Context, repository: R) -> Self {
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
            repository,
            file_path: "hakoniwa.ibcad".into(),
            selected: None,
            piece_editor: PieceEditorState::default(),
            tree_editor: TreeEditorState::default(),
            assembly_editor: AssemblyEditorState::default(),
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
impl<R> eframe::App for HakoniwaApp<R>
where
    R: ProjectRepository + 'static,
    R::Error: Debug,
{
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        ui.painter()
            .rect_filled(ui.max_rect(), 0.0, egui::Color32::WHITE);
        ui.horizontal(|ui| {
            if ui.button("ハンマーを新規作成").clicked() {
                self.editor = Editor::new(hammer_project());
                self.selected = None;
                self.piece_editor.reset();
                self.assembly_editor.reset_camera();
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
                self.piece_editor.finish_drag();
                self.status = "操作を元に戻しました".into();
            }
            if redo_requested && self.editor.redo() {
                self.piece_editor.finish_drag();
                self.status = "操作をやり直しました".into();
            }
            ui.separator();
            ui.label("ファイル");
            ui.add(egui::TextEdit::singleline(&mut self.file_path).desired_width(180.0));
            if ui.button("保存").clicked() {
                let path = ibcad_path(&self.file_path);
                match self.repository.save(&path, self.editor.project()) {
                    Ok(()) => {
                        self.file_path = path.to_string_lossy().into_owned();
                        self.status = format!("保存しました: {}", path.display());
                    }
                    Err(error) => self.status = format!("保存できません: {error:?}"),
                }
            }
            if ui.button("読込").clicked() {
                let path = ibcad_path(&self.file_path);
                match self.repository.load(&path) {
                    Ok(project) => {
                        self.editor = Editor::new(project);
                        self.selected = None;
                        self.piece_editor.reset();
                        self.assembly_editor.reset_camera();
                        self.file_path = path.to_string_lossy().into_owned();
                        self.status = format!("読み込みました: {}", path.display());
                    }
                    Err(error) => self.status = format!("読み込めません: {error:?}"),
                }
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

        let (commands, begin_transaction, mut end_transaction) = {
            let mut behavior = WorkspaceBehavior {
                project: self.editor.project(),
                selected: &mut self.selected,
                piece_editor: &mut self.piece_editor,
                tree_editor: &mut self.tree_editor,
                assembly_editor: &mut self.assembly_editor,
                zoom: &mut self.zoom,
                orientation: &mut self.orientation,
                pan: &mut self.pan,
                perspective: &mut self.perspective,
                close_requested: None,
                commands: Vec::new(),
                begin_transaction: false,
                end_transaction: false,
            };
            self.workspace.ui(&mut behavior, ui);
            if let Some(tile_id) = behavior.close_requested {
                self.workspace.tiles.set_visible(tile_id, false);
            }
            (
                behavior.commands,
                behavior.begin_transaction,
                behavior.end_transaction,
            )
        };

        if !ui.input(|input| input.pointer.primary_down())
            && self.piece_editor.drag_transaction_active
        {
            self.piece_editor.finish_drag();
            end_transaction = true;
        }

        if begin_transaction {
            self.editor.begin_transaction();
        }
        for pending in commands {
            let deletes_selection = matches!(
                &pending.command,
                Command::DeleteObject { object } if Some(*object) == self.selected
            );
            match self.editor.execute(pending.command) {
                Ok(result) => {
                    if pending.select_created && result.created_object.is_some() {
                        self.selected = result.created_object;
                    } else if deletes_selection {
                        self.selected = None;
                        self.piece_editor.reset();
                    }
                    self.status = "編集しました".into();
                }
                Err(error) => self.status = format!("編集できません: {error:?}"),
            }
        }
        if end_transaction {
            self.editor.end_transaction();
        }
    }
}

fn ibcad_path(input: &str) -> PathBuf {
    let mut path = PathBuf::from(input.trim());
    if path.extension().is_none() {
        path.set_extension("ibcad");
    }
    path
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
    assembly_editor: &'a mut AssemblyEditorState,
    zoom: &'a mut f32,
    orientation: &'a mut Quat,
    pan: &'a mut egui::Vec2,
    perspective: &'a mut bool,
    close_requested: Option<egui_tiles::TileId>,
    commands: Vec<PendingCommand>,
    begin_transaction: bool,
    end_transaction: bool,
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
                &mut self.begin_transaction,
                &mut self.end_transaction,
            ),
            WorkspacePane::AssemblyView => draw_preview(
                ui,
                self.project,
                PreviewContext {
                    selected: self.selected,
                    editor: self.assembly_editor,
                    commands: &mut self.commands,
                    zoom: self.zoom,
                    orientation: self.orientation,
                    pan: self.pan,
                    perspective: self.perspective,
                },
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
    renaming: Option<ObjectRef>,
    rename_buffer: String,
    focus_rename: bool,
    dragged: Option<ObjectRef>,
    drop_target: Option<TreeDropTarget>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TreeDropTarget {
    IntoGroup(ObjectId),
    After(ObjectRef),
}

impl TreeEditorState {
    fn start_rename(&mut self, object: ObjectRef, current_name: &str) {
        self.renaming = Some(object);
        self.rename_buffer = current_name.to_owned();
        self.focus_rename = true;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TransformShortcut {
    Move,
    Rotate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MoveConstraint {
    ViewPlane,
    Axis(GridAxis),
    PlaneExcluding(GridAxis),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MoveSource {
    Keyboard,
    Gizmo,
}

#[derive(Clone, Copy)]
struct MoveSession {
    object: ObjectRef,
    start_placement: Placement,
    preview_placement: Placement,
    start_pointer: egui::Pos2,
    constraint: MoveConstraint,
    camera: Option<Camera>,
    source: MoveSource,
}

#[derive(Clone, Copy)]
struct RotateSession {
    object: ObjectRef,
    axis: GridAxis,
    pivot_world: GridPosition,
    center: egui::Pos2,
    start_parameter: f32,
    current_angle: f32,
    quarter_turns: i8,
    camera: Camera,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum TransformTool {
    #[default]
    Move,
    Rotate,
}

#[derive(Default)]
struct AssemblyEditorState {
    shortcut: Option<TransformShortcut>,
    tool: TransformTool,
    move_session: Option<MoveSession>,
    rotate_session: Option<RotateSession>,
    last_camera: Option<Camera>,
    camera_frame: Option<CameraFrame>,
    hovered_object: Option<ObjectRef>,
}

impl AssemblyEditorState {
    fn reset(&mut self) {
        self.shortcut = None;
        self.move_session = None;
        self.rotate_session = None;
    }

    fn reset_camera(&mut self) {
        self.reset();
        self.last_camera = None;
        self.camera_frame = None;
        self.hovered_object = None;
    }
}

struct PieceEditorState {
    piece_id: Option<ObjectId>,
    layout: PlateLayout,
    viewport: Option<Viewport2d>,
    tool: PieceTool,
    color: Color,
    last_dragged_cell: Option<GridPoint2d>,
    drag_transaction_active: bool,
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
            drag_transaction_active: false,
        }
    }
}

impl PieceEditorState {
    fn reset(&mut self) {
        self.piece_id = None;
        self.layout = PlateLayout::empty_piece();
        self.viewport = None;
        self.last_dragged_cell = None;
        self.drag_transaction_active = false;
    }

    fn finish_drag(&mut self) {
        self.last_dragged_cell = None;
        self.drag_transaction_active = false;
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
    handle_tree_shortcuts(ui, project, *selected, state, commands);
    if state.dragged.is_some() {
        state.drop_target = None;
    }
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
        if ui.button("＋パーツ").clicked() {
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
    draw_group_tree(
        ui,
        project,
        project.root_group_id(),
        selected,
        state,
        commands,
    );
    if let Some(object) = state.dragged {
        draw_tree_drag_ghost(ui, project, object);
    }
    if ui.input(|input| input.pointer.any_released()) {
        if let (Some(object), Some(target)) = (state.dragged, state.drop_target) {
            let command = match target {
                TreeDropTarget::IntoGroup(group_id) if can_reparent(project, object, group_id) => {
                    Some(Command::ReparentObject {
                        object,
                        new_parent_group_id: group_id,
                    })
                }
                TreeDropTarget::After(target) if can_move_after(project, object, target) => {
                    Some(Command::MoveObjectAfter { object, target })
                }
                _ => None,
            };
            if let Some(command) = command {
                commands.push(PendingCommand {
                    command,
                    select_created: false,
                });
            }
        }
        state.dragged = None;
        state.drop_target = None;
    }
    ui.separator();
    ui.label(format!("総ビーズ数: {}", project.inventory().total));
}

fn draw_tree_drag_ghost(ui: &egui::Ui, project: &Project, object: ObjectRef) {
    let Some(pointer) = ui.input(|input| input.pointer.interact_pos()) else {
        return;
    };
    let Some(label) = object_tree_label(project, object) else {
        return;
    };

    let mut painter = ui.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::new("parts-tree-drag-ghost"),
    ));
    painter.set_opacity(0.68);
    let font = egui::FontId::proportional(14.0);
    let text_color = ui.visuals().text_color();
    let text_width = painter
        .layout_no_wrap(label.clone(), font.clone(), text_color)
        .size()
        .x;
    let rect = egui::Rect::from_min_size(
        pointer + egui::vec2(12.0, 12.0),
        egui::vec2((text_width + 35.0).max(88.0), 24.0),
    );
    painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(240, 244, 249));
    draw_object_icon(&painter, object, rect.left_center());
    painter.text(
        egui::pos2(rect.left() + 23.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        font,
        text_color,
    );
}

fn object_tree_label(project: &Project, object: ObjectRef) -> Option<String> {
    match object {
        ObjectRef::Group(id) => project.groups.get(&id).map(|group| group.name.clone()),
        ObjectRef::Shape(id) => project
            .shapes
            .get(&id)
            .map(|shape| format!("{} ({} beads)", shape.name, shape.beads.len())),
        ObjectRef::Piece(id) => project
            .pieces
            .get(&id)
            .map(|piece| format!("{} ({} beads)", piece.name, piece.beads.len())),
    }
}

fn handle_tree_shortcuts(
    ui: &egui::Ui,
    project: &Project,
    selected: Option<ObjectRef>,
    state: &mut TreeEditorState,
    commands: &mut Vec<PendingCommand>,
) {
    if ui.ctx().egui_wants_keyboard_input() {
        return;
    }
    if ui.input(|input| input.key_pressed(egui::Key::F2))
        && let Some(object) = selected
        && let Some(name) = object_name(project, object)
    {
        state.start_rename(object, name);
    }
    if ui.input(|input| input.key_pressed(egui::Key::Delete))
        && let Some(object) = selected
        && can_delete(project, object)
    {
        commands.push(PendingCommand {
            command: Command::DeleteObject { object },
            select_created: false,
        });
    }
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
    state: &mut TreeEditorState,
    commands: &mut Vec<PendingCommand>,
) {
    let Some(group) = project.groups.get(&group_id) else {
        return;
    };
    draw_object_row(
        ui,
        &group.name,
        ObjectRef::Group(group.id),
        group.visible,
        &mut TreeRowContext {
            project,
            selected,
            state,
            commands,
        },
    );
    ui.indent(("group-children", group.id), |ui| {
        for child in project.child_objects(group.id) {
            match child {
                ObjectRef::Group(id) => {
                    draw_group_tree(ui, project, id, selected, state, commands);
                }
                ObjectRef::Shape(id) => {
                    let shape = &project.shapes[&id];
                    draw_object_row(
                        ui,
                        &format!("{} ({} beads)", shape.name, shape.beads.len()),
                        child,
                        shape.visible,
                        &mut TreeRowContext {
                            project,
                            selected,
                            state,
                            commands,
                        },
                    );
                }
                ObjectRef::Piece(id) => {
                    let piece = &project.pieces[&id];
                    draw_object_row(
                        ui,
                        &format!("{} ({} beads)", piece.name, piece.beads.len()),
                        child,
                        piece.visible,
                        &mut TreeRowContext {
                            project,
                            selected,
                            state,
                            commands,
                        },
                    );
                }
            }
        }
    });
}

struct TreeRowContext<'a> {
    project: &'a Project,
    selected: &'a mut Option<ObjectRef>,
    state: &'a mut TreeEditorState,
    commands: &'a mut Vec<PendingCommand>,
}

fn draw_object_row(
    ui: &mut egui::Ui,
    label: &str,
    object: ObjectRef,
    visible: bool,
    context: &mut TreeRowContext<'_>,
) {
    let row_height = 24.0;
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), row_height),
        egui::Sense::hover(),
    );
    let eye_rect = egui::Rect::from_min_max(
        egui::pos2(rect.right() - row_height, rect.top()),
        rect.right_bottom(),
    );
    let main_rect = egui::Rect::from_min_max(rect.min, egui::pos2(eye_rect.left(), rect.bottom()));
    let row_id = ui.id().with((object_kind_id(object), object.id()));

    if context.state.renaming == Some(object) {
        let response = ui.put(
            main_rect.shrink2(egui::vec2(2.0, 1.0)),
            egui::TextEdit::singleline(&mut context.state.rename_buffer),
        );
        if context.state.focus_rename {
            response.request_focus();
            context.state.focus_rename = false;
        }
        let cancel = ui.input(|input| input.key_pressed(egui::Key::Escape));
        let commit = ui.input(|input| input.key_pressed(egui::Key::Enter))
            || (response.lost_focus() && !cancel);
        if cancel {
            context.state.renaming = None;
        } else if commit {
            let name = context.state.rename_buffer.trim();
            if !name.is_empty() && object_name(context.project, object) != Some(name) {
                context.commands.push(PendingCommand {
                    command: Command::RenameObject {
                        object,
                        name: name.to_owned(),
                    },
                    select_created: false,
                });
            }
            context.state.renaming = None;
        }
    } else {
        let response = ui.interact(
            main_rect,
            row_id.with("main"),
            egui::Sense::click_and_drag(),
        );
        let hovered_drop_target = context.state.dragged.and_then(|dragged| {
            if !response.contains_pointer() {
                return None;
            }
            let pointer = ui.input(|input| input.pointer.hover_pos())?;
            if pointer.y >= main_rect.top() + main_rect.height() * 0.6
                && can_move_after(context.project, dragged, object)
            {
                Some(TreeDropTarget::After(object))
            } else if let ObjectRef::Group(id) = object
                && can_reparent(context.project, dragged, id)
            {
                Some(TreeDropTarget::IntoGroup(id))
            } else {
                None
            }
        });
        let background = if matches!(hovered_drop_target, Some(TreeDropTarget::After(_))) {
            egui::Color32::from_rgb(225, 238, 252)
        } else if matches!(hovered_drop_target, Some(TreeDropTarget::IntoGroup(_))) {
            egui::Color32::from_rgb(190, 215, 245)
        } else if *context.selected == Some(object) {
            egui::Color32::from_rgb(205, 225, 248)
        } else if response.hovered() {
            egui::Color32::from_gray(235)
        } else {
            egui::Color32::TRANSPARENT
        };
        ui.painter().rect_filled(main_rect, 2.0, background);
        draw_object_icon(ui.painter(), object, main_rect.left_center());
        ui.painter().text(
            egui::pos2(main_rect.left() + 23.0, main_rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(14.0),
            ui.visuals().text_color(),
        );
        if matches!(hovered_drop_target, Some(TreeDropTarget::After(_))) {
            ui.painter().line_segment(
                [
                    egui::pos2(main_rect.left() + 2.0, main_rect.bottom() - 3.0),
                    egui::pos2(main_rect.right() - 2.0, main_rect.bottom() - 3.0),
                ],
                egui::Stroke::new(4.0, egui::Color32::from_rgb(35, 105, 205)),
            );
        }

        if response.clicked() {
            *context.selected = Some(object);
        }
        if response.drag_started() {
            *context.selected = Some(object);
            context.state.dragged = Some(object);
        }
        if let Some(target) = hovered_drop_target {
            context.state.drop_target = Some(target);
        }
        response.context_menu(|ui| {
            if ui.button("名前を変更    F2").clicked() {
                if let Some(name) = object_name(context.project, object) {
                    context.state.start_rename(object, name);
                }
                ui.close();
            }
            if let ObjectRef::Group(parent_group_id) = object {
                ui.separator();
                if ui.button("子パーツを追加").clicked() {
                    queue_child_group(context.project, parent_group_id, context.commands);
                    ui.close();
                }
                ui.menu_button("子Pieceを追加", |ui| {
                    for plane in [NewPiecePlane::Xy, NewPiecePlane::Xz, NewPiecePlane::Yz] {
                        if ui.button(plane.label()).clicked() {
                            queue_child_piece(
                                context.project,
                                parent_group_id,
                                plane,
                                context.commands,
                            );
                            ui.close();
                        }
                    }
                });
            }
            ui.separator();
            if ui
                .add_enabled(
                    can_delete(context.project, object),
                    egui::Button::new("削除    Delete"),
                )
                .clicked()
            {
                context.commands.push(PendingCommand {
                    command: Command::DeleteObject { object },
                    select_created: false,
                });
                ui.close();
            }
        });
    }

    let eye_response = ui.interact(eye_rect, row_id.with("visibility"), egui::Sense::click());
    if eye_response.clicked() {
        context.commands.push(PendingCommand {
            command: Command::SetVisibility {
                object,
                visible: !visible,
            },
            select_created: false,
        });
    }
    if eye_response.hovered() {
        ui.painter()
            .rect_filled(eye_rect, 2.0, egui::Color32::from_gray(235));
    }
    draw_eye_icon(ui.painter(), eye_rect.center(), visible);
}

const fn object_kind_id(object: ObjectRef) -> u8 {
    match object {
        ObjectRef::Group(_) => 0,
        ObjectRef::Shape(_) => 1,
        ObjectRef::Piece(_) => 2,
    }
}

fn object_name(project: &Project, object: ObjectRef) -> Option<&str> {
    match object {
        ObjectRef::Group(id) => project.groups.get(&id).map(|group| group.name.as_str()),
        ObjectRef::Shape(id) => project.shapes.get(&id).map(|shape| shape.name.as_str()),
        ObjectRef::Piece(id) => project.pieces.get(&id).map(|piece| piece.name.as_str()),
    }
}

fn object_parent_group(project: &Project, object: ObjectRef) -> Option<ObjectId> {
    match object {
        ObjectRef::Group(id) => project.groups.get(&id)?.parent_group_id,
        ObjectRef::Shape(id) => Some(project.shapes.get(&id)?.parent_group_id),
        ObjectRef::Piece(id) => Some(project.pieces.get(&id)?.parent_group_id),
    }
}

fn can_delete(project: &Project, object: ObjectRef) -> bool {
    match object {
        ObjectRef::Group(id) => {
            id != project.root_group_id()
                && !project
                    .groups
                    .values()
                    .any(|group| group.parent_group_id == Some(id))
                && !project
                    .shapes
                    .values()
                    .any(|shape| shape.parent_group_id == id)
                && !project
                    .pieces
                    .values()
                    .any(|piece| piece.parent_group_id == id)
        }
        ObjectRef::Shape(id) => project.shapes.contains_key(&id),
        ObjectRef::Piece(id) => project.pieces.contains_key(&id),
    }
}

fn can_reparent(project: &Project, object: ObjectRef, target_group_id: ObjectId) -> bool {
    if !project.groups.contains_key(&target_group_id)
        || object_parent_group(project, object) == Some(target_group_id)
    {
        return false;
    }
    match object {
        ObjectRef::Group(id) => {
            if id == project.root_group_id() || id == target_group_id {
                return false;
            }
            let mut ancestor = Some(target_group_id);
            while let Some(group_id) = ancestor {
                if group_id == id {
                    return false;
                }
                ancestor = project
                    .groups
                    .get(&group_id)
                    .and_then(|group| group.parent_group_id);
            }
            true
        }
        ObjectRef::Shape(id) => project.shapes.contains_key(&id),
        ObjectRef::Piece(id) => project.pieces.contains_key(&id),
    }
}

fn can_move_after(project: &Project, object: ObjectRef, target: ObjectRef) -> bool {
    if object == target {
        return false;
    }
    let Some(target_parent) = object_parent_group(project, target) else {
        return false;
    };
    match object {
        ObjectRef::Group(id) => {
            if id == project.root_group_id() || !project.groups.contains_key(&id) {
                return false;
            }
            let mut ancestor = Some(target_parent);
            while let Some(group_id) = ancestor {
                if group_id == id {
                    return false;
                }
                ancestor = project
                    .groups
                    .get(&group_id)
                    .and_then(|group| group.parent_group_id);
            }
            true
        }
        ObjectRef::Shape(id) => project.shapes.contains_key(&id),
        ObjectRef::Piece(id) => project.pieces.contains_key(&id),
    }
}

fn queue_child_group(
    project: &Project,
    parent_group_id: ObjectId,
    commands: &mut Vec<PendingCommand>,
) {
    commands.push(PendingCommand {
        command: Command::CreateGroup {
            parent_group_id,
            name: format!("Group {}", project.groups.len()),
        },
        select_created: true,
    });
}

fn queue_child_piece(
    project: &Project,
    parent_group_id: ObjectId,
    plane: NewPiecePlane,
    commands: &mut Vec<PendingCommand>,
) {
    commands.push(PendingCommand {
        command: Command::CreatePiece {
            parent_group_id,
            name: format!("Piece {}", project.pieces.len() + 1),
            plane: plane.plane(),
        },
        select_created: true,
    });
}

fn draw_object_icon(painter: &egui::Painter, object: ObjectRef, left_center: egui::Pos2) {
    let origin = egui::pos2(left_center.x + 3.0, left_center.y - 7.0);
    match object {
        ObjectRef::Group(_) => {
            let color = egui::Color32::from_rgb(218, 164, 52);
            painter.rect_filled(
                egui::Rect::from_min_size(origin, egui::vec2(8.0, 4.0)),
                1.0,
                color,
            );
            painter.rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(origin.x, origin.y + 3.0),
                    egui::vec2(17.0, 12.0),
                ),
                2.0,
                color,
            );
        }
        ObjectRef::Shape(_) => {
            let center = egui::pos2(origin.x + 8.0, origin.y + 7.0);
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(center.x, center.y - 7.0),
                    egui::pos2(center.x + 7.0, center.y),
                    egui::pos2(center.x, center.y + 7.0),
                    egui::pos2(center.x - 7.0, center.y),
                ],
                egui::Color32::from_rgb(118, 145, 190),
                egui::Stroke::NONE,
            ));
        }
        ObjectRef::Piece(_) => {
            let color = egui::Color32::from_rgb(94, 143, 197);
            for row in 0..3 {
                for column in 0..3 {
                    painter.rect_filled(
                        egui::Rect::from_min_size(
                            egui::pos2(origin.x + column as f32 * 5.5, origin.y + row as f32 * 5.5),
                            egui::vec2(4.0, 4.0),
                        ),
                        0.8,
                        color,
                    );
                }
            }
        }
    }
}

fn draw_eye_icon(painter: &egui::Painter, center: egui::Pos2, visible: bool) {
    let color = if visible {
        egui::Color32::from_gray(75)
    } else {
        egui::Color32::from_gray(175)
    };
    let left = egui::pos2(center.x - 8.0, center.y);
    let right = egui::pos2(center.x + 8.0, center.y);
    painter.line_segment(
        [left, egui::pos2(center.x, center.y - 5.0)],
        egui::Stroke::new(1.5, color),
    );
    painter.line_segment(
        [egui::pos2(center.x, center.y - 5.0), right],
        egui::Stroke::new(1.5, color),
    );
    painter.line_segment(
        [right, egui::pos2(center.x, center.y + 5.0)],
        egui::Stroke::new(1.5, color),
    );
    painter.line_segment(
        [egui::pos2(center.x, center.y + 5.0), left],
        egui::Stroke::new(1.5, color),
    );
    if visible {
        painter.circle_filled(center, 2.5, color);
    } else {
        painter.line_segment(
            [
                egui::pos2(center.x - 7.0, center.y + 7.0),
                egui::pos2(center.x + 7.0, center.y - 7.0),
            ],
            egui::Stroke::new(2.0, color),
        );
    }
}

fn draw_editor(
    ui: &mut egui::Ui,
    piece: Option<&Piece>,
    state: &mut PieceEditorState,
    commands: &mut Vec<PendingCommand>,
    begin_transaction: &mut bool,
    end_transaction: &mut bool,
) {
    let Some(piece) = piece else {
        if state.drag_transaction_active {
            *end_transaction = true;
        }
        state.reset();
        ui.label("ツリーからPieceを選択してください");
        return;
    };

    if state.piece_id != Some(piece.id) && state.drag_transaction_active {
        state.finish_drag();
        *end_transaction = true;
    }
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
        if state.drag_transaction_active {
            state.finish_drag();
            *end_transaction = true;
        }
    } else if response.hovered()
        && pointer_cell.is_some()
        && pointer_cell != state.last_dragged_cell
    {
        if !state.drag_transaction_active {
            state.drag_transaction_active = true;
            *begin_transaction = true;
        }
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

struct PreviewContext<'a> {
    selected: &'a mut Option<ObjectRef>,
    editor: &'a mut AssemblyEditorState,
    commands: &'a mut Vec<PendingCommand>,
    zoom: &'a mut f32,
    orientation: &'a mut Quat,
    pan: &'a mut egui::Vec2,
    perspective: &'a mut bool,
}

fn draw_preview(ui: &mut egui::Ui, project: &Project, context: PreviewContext<'_>) {
    handle_assembly_shortcuts(
        ui,
        project,
        *context.selected,
        context.editor,
        context.commands,
    );
    ui.horizontal(|ui| {
        ui.selectable_value(&mut context.editor.tool, TransformTool::Move, "移動ギズモ");
        ui.selectable_value(
            &mut context.editor.tool,
            TransformTool::Rotate,
            "回転ギズモ",
        );
        ui.separator();
        ui.checkbox(context.perspective, "透視投影");
        if ui.button("ビューをリセット").clicked() {
            *context.orientation = Quat::from_rotation_x(-35.0_f32.to_radians())
                * Quat::from_rotation_z(45.0_f32.to_radians());
            *context.zoom = 1.0;
            *context.pan = egui::Vec2::ZERO;
            context.editor.reset_camera();
        }
    });
    if let Some(session) = context.editor.move_session {
        let constraint = move_constraint_label(session.constraint);
        ui.colored_label(
            egui::Color32::from_rgb(210, 105, 15),
            format!(
                "移動 ({constraint}): X/Y/Zで軸固定 · Shift+X/Y/Zで軸除外 · 左クリック/Enterで確定 · 右クリック/Escで取消"
            ),
        );
    } else if let Some(session) = context.editor.rotate_session {
        ui.colored_label(
            axis_color(session.axis),
            format!(
                "{}軸回転: {}° · 左ボタン解放で確定 · 右クリック/Escで取消",
                axis_label(session.axis),
                i16::from(session.quarter_turns) * 90
            ),
        );
    } else if context.editor.shortcut == Some(TransformShortcut::Rotate) {
        ui.colored_label(
            egui::Color32::from_rgb(210, 105, 15),
            "90°回転: X / Y / Zで軸を指定 · Shiftで負方向 · Escで取消",
        );
    }
    ui.label("ギズモ軸をクリック: +1マス / +90° · G→X/Y/Z: 軸固定 · G→Shift+軸: その軸を除外");
    ui.label(
        "中ホイールドラッグ: マウス下を中心にTurntable回転 / Shift+中ホイール: 移動 / ホイール: ズーム",
    );
    let (rect, response) =
        ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
    let orbit_started = response.drag_started_by(egui::PointerButton::Middle)
        && !ui.input(|input| input.modifiers.shift);
    if orbit_started && let Some(object) = context.editor.hovered_object.or(*context.selected) {
        retarget_orbit_center(project, object, rect.center(), context.editor, context.pan);
    }
    if response.hovered() {
        ui.input(|input| {
            if input.pointer.button_down(egui::PointerButton::Middle) {
                let delta = input.pointer.delta();
                if input.modifiers.shift {
                    *context.pan += delta;
                } else {
                    apply_turntable_drag(context.orientation, delta);
                }
            }
            if input.smooth_scroll_delta.y != 0.0 {
                *context.zoom =
                    (*context.zoom * (1.0 + input.smooth_scroll_delta.y * 0.001)).clamp(0.1, 8.0);
            }
        });
    }
    let raw_pointer = ui.input(|input| input.pointer.hover_pos());
    let pointer = raw_pointer.filter(|pointer| rect.contains(*pointer));
    if let (Some(session), Some(pointer)) = (&mut context.editor.move_session, pointer) {
        update_move_preview(
            project,
            session,
            pointer,
            rect.center(),
            context.editor.last_camera,
        );
    }
    if let (Some(session), Some(pointer)) = (&mut context.editor.rotate_session, raw_pointer) {
        update_rotate_preview(session, pointer);
    }
    let preview_project = context
        .editor
        .move_session
        .map(|session| {
            let mut preview = project.clone();
            preview
                .set_placement(session.object, session.preview_placement)
                .expect("move preview object must exist");
            preview
        })
        .or_else(|| {
            context.editor.rotate_session.map(|session| {
                let mut preview = project.clone();
                preview
                    .rotate_object_quarter_around_world(
                        session.object,
                        session.axis,
                        session.quarter_turns,
                        session.pivot_world,
                    )
                    .expect("rotate preview object must exist");
                preview
            })
        });
    let render_project = preview_project.as_ref().unwrap_or(project);
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, egui::Color32::WHITE);
    let rendered = render_voxels(
        &painter,
        rect,
        render_project,
        RenderOptions {
            selected: *context.selected,
            orientation: *context.orientation,
            zoom: *context.zoom,
            pan: *context.pan,
            perspective: *context.perspective,
            camera_frame: context.editor.camera_frame,
        },
    );
    context.editor.last_camera = rendered.camera;
    if context.editor.camera_frame.is_none() {
        context.editor.camera_frame = rendered.camera_frame;
    }
    context.editor.hovered_object =
        pointer.and_then(|pointer| pick_rendered_object(&rendered.faces, pointer));
    if let (Some(session), Some(camera)) = (context.editor.move_session, rendered.camera.as_ref()) {
        draw_move_constraint_guide(&painter, rect, render_project, session, camera);
    }
    if let Some(session) = context.editor.rotate_session {
        draw_rotation_drag_indicator(&painter, session);
    }
    let transform_was_active =
        context.editor.move_session.is_some() || context.editor.rotate_session.is_some();
    let confirm_move = context
        .editor
        .move_session
        .is_some_and(|session| match session.source {
            MoveSource::Keyboard => {
                response.clicked_by(egui::PointerButton::Primary)
                    || ui.input(|input| input.key_pressed(egui::Key::Enter))
            }
            MoveSource::Gizmo => {
                ui.input(|input| input.pointer.button_released(egui::PointerButton::Primary))
            }
        });
    let confirm_rotate = context.editor.rotate_session.is_some()
        && ui.input(|input| input.pointer.button_released(egui::PointerButton::Primary));
    let cancel_transform =
        transform_was_active && response.clicked_by(egui::PointerButton::Secondary);
    if confirm_move {
        let session = context
            .editor
            .move_session
            .take()
            .expect("active move session must exist");
        context.editor.shortcut = None;
        if session.preview_placement != session.start_placement {
            context.commands.push(PendingCommand {
                command: Command::SetPlacement {
                    object: session.object,
                    placement: session.preview_placement,
                },
                select_created: false,
            });
        }
    } else if confirm_rotate {
        let session = context
            .editor
            .rotate_session
            .take()
            .expect("active rotate session must exist");
        context.editor.shortcut = None;
        if session.quarter_turns != 0 {
            context.commands.push(PendingCommand {
                command: Command::RotateQuarterAround {
                    object: session.object,
                    axis: session.axis,
                    quarter_turns: session.quarter_turns,
                    pivot_world: session.pivot_world,
                },
                select_created: false,
            });
        }
    } else if cancel_transform {
        context.editor.reset();
    }
    let gizmo_handled = !transform_was_active
        && draw_transform_gizmo(GizmoContext {
            ui,
            painter: &painter,
            rect,
            pointer,
            project,
            selected: *context.selected,
            tool: context.editor.tool,
            camera: rendered.camera.as_ref(),
            clicked: response.clicked_by(egui::PointerButton::Primary),
            drag_started: response.drag_started_by(egui::PointerButton::Primary),
            editor: context.editor,
            commands: context.commands,
        });
    if !transform_was_active
        && response.clicked_by(egui::PointerButton::Primary)
        && !gizmo_handled
        && let Some(pointer) = response.interact_pointer_pos()
    {
        *context.selected = pick_rendered_object(&rendered.faces, pointer);
    }
}

struct GizmoContext<'a> {
    ui: &'a egui::Ui,
    painter: &'a egui::Painter,
    rect: egui::Rect,
    pointer: Option<egui::Pos2>,
    project: &'a Project,
    selected: Option<ObjectRef>,
    tool: TransformTool,
    camera: Option<&'a Camera>,
    clicked: bool,
    drag_started: bool,
    editor: &'a mut AssemblyEditorState,
    commands: &'a mut Vec<PendingCommand>,
}

const MOVE_GIZMO_LENGTH: f32 = 64.0;
const ROTATE_GIZMO_RADIUS: f32 = 48.0;

fn draw_transform_gizmo(context: GizmoContext<'_>) -> bool {
    let (Some(object), Some(camera)) = (context.selected, context.camera) else {
        return false;
    };
    let Some(origin) = (match context.tool {
        TransformTool::Move => object_gizmo_center(context.project, object),
        TransformTool::Rotate => object_rotation_pivot(context.project, object).map(grid_to_vec3),
    }) else {
        return false;
    };
    let center = world_to_screen(context.rect.center(), origin, camera);
    let axes = [GridAxis::X, GridAxis::Y, GridAxis::Z];
    let hovered = match context.tool {
        TransformTool::Move => axes.into_iter().find(|axis| {
            context.pointer.is_some_and(|pointer| {
                projected_axis_direction(context.rect.center(), camera, origin, *axis).is_some_and(
                    |direction| {
                        let end = center + direction * MOVE_GIZMO_LENGTH;
                        distance_to_segment(pointer, center, end) <= 7.0
                    },
                )
            })
        }),
        TransformTool::Rotate => axes.into_iter().find(|axis| {
            let ring = rotation_ring_points(center, camera, *axis);
            context
                .pointer
                .is_some_and(|pointer| distance_to_polyline(pointer, &ring) <= 7.0)
        }),
    };

    for axis in axes {
        let color = axis_color(axis);
        let width = match context.tool {
            TransformTool::Move if hovered == Some(axis) => 4.5,
            TransformTool::Move => 3.0,
            TransformTool::Rotate if hovered == Some(axis) => 2.75,
            TransformTool::Rotate => 1.5,
        };
        match context.tool {
            TransformTool::Move => {
                let Some(direction) =
                    projected_axis_direction(context.rect.center(), camera, origin, axis)
                else {
                    continue;
                };
                let end = center + direction * MOVE_GIZMO_LENGTH;
                context
                    .painter
                    .line_segment([center, end], egui::Stroke::new(width, color));
                context.painter.circle_filled(end, 5.0, color);
                context.painter.text(
                    end + egui::vec2(7.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    axis_label(axis),
                    egui::FontId::proportional(12.0),
                    color,
                );
            }
            TransformTool::Rotate => {
                let ring = rotation_ring_points(center, camera, axis);
                context
                    .painter
                    .add(egui::Shape::line(ring, egui::Stroke::new(width, color)));
            }
        }
    }
    context
        .painter
        .circle_filled(center, 4.0, egui::Color32::from_rgb(245, 245, 245));

    if context.tool == TransformTool::Move
        && context.drag_started
        && let Some(axis) = hovered
        && let Some(start_placement) = placement_for_object(context.project, object)
        && let Some(start_pointer) = context
            .ui
            .input(|input| input.pointer.press_origin())
            .or(context.pointer)
    {
        context.editor.shortcut = Some(TransformShortcut::Move);
        context.editor.move_session = Some(MoveSession {
            object,
            start_placement,
            preview_placement: start_placement,
            start_pointer,
            constraint: MoveConstraint::Axis(axis),
            camera: Some(*camera),
            source: MoveSource::Gizmo,
        });
        return true;
    }

    if context.tool == TransformTool::Rotate
        && context.drag_started
        && let Some(axis) = hovered
        && let Some(pivot_world) = object_rotation_pivot(context.project, object)
        && let Some(start_pointer) = context
            .ui
            .input(|input| input.pointer.press_origin())
            .or(context.pointer)
    {
        let parameter = rotation_ring_parameter(center, camera, axis, start_pointer);
        context.editor.shortcut = Some(TransformShortcut::Rotate);
        context.editor.rotate_session = Some(RotateSession {
            object,
            axis,
            pivot_world,
            center,
            start_parameter: parameter,
            current_angle: 0.0,
            quarter_turns: 0,
            camera: *camera,
        });
        return true;
    }

    if !context.clicked || hovered.is_none() {
        return false;
    }
    let axis = hovered.expect("hovered axis was checked");
    let direction = if context.ui.input(|input| input.modifiers.shift) {
        -1
    } else {
        1
    };
    let command = match context.tool {
        TransformTool::Move => {
            let Some(mut placement) = placement_for_object(context.project, object) else {
                return false;
            };
            match axis {
                GridAxis::X => placement.translation.x += direction,
                GridAxis::Y => placement.translation.y += direction,
                GridAxis::Z => placement.translation.z += direction,
            }
            Command::SetPlacement { object, placement }
        }
        TransformTool::Rotate => {
            let Some(pivot_world) = object_rotation_pivot(context.project, object) else {
                return false;
            };
            Command::RotateQuarterAround {
                object,
                axis,
                quarter_turns: direction as i8,
                pivot_world,
            }
        }
    };
    context.commands.push(PendingCommand {
        command,
        select_created: false,
    });
    true
}

fn object_world_origin(project: &Project, object: ObjectRef) -> Option<Vec3> {
    let position = match object {
        ObjectRef::Group(id) => {
            let group = project.groups.get(&id)?;
            if let Some(parent) = group.parent_group_id {
                transformed_world_position(project, parent, group.placement, GridPosition::ZERO)
            } else {
                group.placement.translation
            }
        }
        ObjectRef::Shape(id) => {
            let shape = project.shapes.get(&id)?;
            transformed_world_position(
                project,
                shape.parent_group_id,
                shape.placement,
                GridPosition::ZERO,
            )
        }
        ObjectRef::Piece(id) => {
            let piece = project.pieces.get(&id)?;
            transformed_world_position(
                project,
                piece.parent_group_id,
                piece.placement,
                GridPosition::ZERO,
            )
        }
    };
    Some(Vec3::new(
        position.x as f32,
        position.y as f32,
        position.z as f32,
    ))
}

fn object_gizmo_center(project: &Project, object: ObjectRef) -> Option<Vec3> {
    let mut minimum: Option<GridPosition> = None;
    let mut maximum: Option<GridPosition> = None;
    let mut include = |position: GridPosition| {
        minimum = Some(minimum.map_or(position, |current| {
            GridPosition::new(
                current.x.min(position.x),
                current.y.min(position.y),
                current.z.min(position.z),
            )
        }));
        maximum = Some(maximum.map_or(position, |current| {
            GridPosition::new(
                current.x.max(position.x),
                current.y.max(position.y),
                current.z.max(position.z),
            )
        }));
    };

    for piece in project.pieces.values().filter(|piece| {
        piece.visible
            && group_chain_is_visible(project, piece.parent_group_id)
            && object_is_selected(project, ObjectRef::Piece(piece.id), Some(object))
    }) {
        for position in piece.beads.keys() {
            include(transformed_world_position(
                project,
                piece.parent_group_id,
                piece.placement,
                *position,
            ));
        }
    }
    for shape in project.shapes.values().filter(|shape| {
        shape.visible
            && group_chain_is_visible(project, shape.parent_group_id)
            && object_is_selected(project, ObjectRef::Shape(shape.id), Some(object))
    }) {
        for position in shape.beads.keys() {
            include(transformed_world_position(
                project,
                shape.parent_group_id,
                shape.placement,
                *position,
            ));
        }
    }

    match (minimum, maximum) {
        (Some(minimum), Some(maximum)) => Some(Vec3::new(
            (minimum.x + maximum.x) as f32 * 0.5,
            (minimum.y + maximum.y) as f32 * 0.5,
            (minimum.z + maximum.z) as f32 * 0.5,
        )),
        _ => object_world_origin(project, object),
    }
}

fn object_rotation_pivot(project: &Project, object: ObjectRef) -> Option<GridPosition> {
    object_gizmo_center(project, object).map(|center| {
        GridPosition::new(
            center.x.round() as i32,
            center.y.round() as i32,
            center.z.round() as i32,
        )
    })
}

const fn grid_to_vec3(position: GridPosition) -> Vec3 {
    Vec3::new(position.x as f32, position.y as f32, position.z as f32)
}

fn world_to_screen(center: egui::Pos2, world: Vec3, camera: &Camera) -> egui::Pos2 {
    project_point(center, camera.orientation * (world - camera.target), camera)
}

fn retarget_orbit_center(
    project: &Project,
    object: ObjectRef,
    viewport_center: egui::Pos2,
    editor: &mut AssemblyEditorState,
    pan: &mut egui::Vec2,
) {
    let (Some(target), Some(camera), Some(mut frame)) = (
        object_gizmo_center(project, object),
        editor.last_camera,
        editor.camera_frame,
    ) else {
        return;
    };
    let target_screen = world_to_screen(viewport_center, target, &camera);
    *pan = target_screen - viewport_center;
    frame.target = target;
    editor.camera_frame = Some(frame);
    editor.last_camera = Some(Camera {
        target,
        pan: *pan,
        ..camera
    });
}

fn draw_move_constraint_guide(
    painter: &egui::Painter,
    rect: egui::Rect,
    project: &Project,
    session: MoveSession,
    camera: &Camera,
) {
    let Some(origin) = object_gizmo_center(project, session.object) else {
        return;
    };
    let center = world_to_screen(rect.center(), origin, camera);
    let extent = rect.width() + rect.height();
    let mut drew_guide = false;
    for axis in [GridAxis::X, GridAxis::Y, GridAxis::Z] {
        let show = match session.constraint {
            MoveConstraint::ViewPlane => false,
            MoveConstraint::Axis(constrained) => axis == constrained,
            MoveConstraint::PlaneExcluding(excluded) => axis != excluded,
        };
        if !show {
            continue;
        }
        let Some(direction) = projected_axis_direction(rect.center(), camera, origin, axis) else {
            continue;
        };
        let direction = direction * extent;
        painter.line_segment(
            [center - direction, center + direction],
            egui::Stroke::new(1.75, axis_color(axis)),
        );
        drew_guide = true;
    }
    if drew_guide {
        painter.circle_filled(center, 4.5, egui::Color32::from_rgb(245, 245, 245));
    }
}

fn projected_axis_direction(
    viewport_center: egui::Pos2,
    camera: &Camera,
    origin: Vec3,
    axis: GridAxis,
) -> Option<egui::Vec2> {
    let start = world_to_screen(viewport_center, origin, camera);
    let end = world_to_screen(viewport_center, origin + axis_vector(axis), camera);
    let direction = end - start;
    (direction.length_sq() > f32::EPSILON).then(|| direction.normalized())
}

fn rotation_ring_points(center: egui::Pos2, camera: &Camera, axis: GridAxis) -> Vec<egui::Pos2> {
    (0..=64)
        .map(|index| {
            rotation_ring_point(
                center,
                camera,
                axis,
                index as f32 / 64.0 * std::f32::consts::TAU,
            )
        })
        .collect()
}

fn rotation_ring_point(
    center: egui::Pos2,
    camera: &Camera,
    axis: GridAxis,
    angle: f32,
) -> egui::Pos2 {
    let (first, second) = match axis {
        GridAxis::X => (Vec3::Y, Vec3::Z),
        GridAxis::Y => (Vec3::Z, Vec3::X),
        GridAxis::Z => (Vec3::X, Vec3::Y),
    };
    let camera_point = camera.orientation * (first * angle.cos() + second * angle.sin());
    center + egui::vec2(camera_point.x, -camera_point.y) * ROTATE_GIZMO_RADIUS
}

fn rotation_ring_parameter(
    center: egui::Pos2,
    camera: &Camera,
    axis: GridAxis,
    pointer: egui::Pos2,
) -> f32 {
    rotation_ring_points(center, camera, axis)
        .into_iter()
        .take(64)
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            left.distance_sq(pointer)
                .total_cmp(&right.distance_sq(pointer))
        })
        .map_or(0.0, |(index, _)| {
            index as f32 / 64.0 * std::f32::consts::TAU
        })
}

const fn axis_vector(axis: GridAxis) -> Vec3 {
    match axis {
        GridAxis::X => Vec3::X,
        GridAxis::Y => Vec3::Y,
        GridAxis::Z => Vec3::Z,
    }
}

const fn axis_color(axis: GridAxis) -> egui::Color32 {
    match axis {
        GridAxis::X => egui::Color32::from_rgb(210, 55, 55),
        GridAxis::Y => egui::Color32::from_rgb(45, 160, 75),
        GridAxis::Z => egui::Color32::from_rgb(55, 105, 220),
    }
}

const fn axis_label(axis: GridAxis) -> &'static str {
    match axis {
        GridAxis::X => "X",
        GridAxis::Y => "Y",
        GridAxis::Z => "Z",
    }
}

fn distance_to_polyline(point: egui::Pos2, points: &[egui::Pos2]) -> f32 {
    points
        .windows(2)
        .map(|segment| distance_to_segment(point, segment[0], segment[1]))
        .fold(f32::INFINITY, f32::min)
}

fn distance_to_segment(point: egui::Pos2, start: egui::Pos2, end: egui::Pos2) -> f32 {
    let segment = end - start;
    let length_squared = segment.length_sq();
    if length_squared <= f32::EPSILON {
        return point.distance(start);
    }
    let fraction = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
    point.distance(start + segment * fraction)
}

fn handle_assembly_shortcuts(
    ui: &egui::Ui,
    project: &Project,
    selected: Option<ObjectRef>,
    state: &mut AssemblyEditorState,
    commands: &mut Vec<PendingCommand>,
) {
    if ui.ctx().egui_wants_keyboard_input() {
        return;
    }
    if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
        state.reset();
        return;
    }
    if state.rotate_session.is_some() {
        return;
    }
    if ui.input(|input| input.key_pressed(egui::Key::G))
        && let Some(object) = selected
        && let Some(start_placement) = placement_for_object(project, object)
        && let Some(start_pointer) = ui.input(|input| input.pointer.hover_pos())
    {
        state.shortcut = Some(TransformShortcut::Move);
        state.tool = TransformTool::Move;
        state.move_session = Some(MoveSession {
            object,
            start_placement,
            preview_placement: start_placement,
            start_pointer,
            constraint: MoveConstraint::ViewPlane,
            camera: state.last_camera,
            source: MoveSource::Keyboard,
        });
        return;
    }
    if let Some(session) = &mut state.move_session {
        let constraint = ui.input(|input| {
            let axis = if input.key_pressed(egui::Key::X) {
                Some(GridAxis::X)
            } else if input.key_pressed(egui::Key::Y) {
                Some(GridAxis::Y)
            } else if input.key_pressed(egui::Key::Z) {
                Some(GridAxis::Z)
            } else {
                None
            }?;
            Some(if input.modifiers.shift {
                MoveConstraint::PlaneExcluding(axis)
            } else {
                MoveConstraint::Axis(axis)
            })
        });
        if let Some(constraint) = constraint {
            session.constraint = constraint;
        }
        return;
    }
    if ui.input(|input| input.key_pressed(egui::Key::R)) && selected.is_some() {
        state.shortcut = Some(TransformShortcut::Rotate);
        state.tool = TransformTool::Rotate;
        return;
    }
    let Some(shortcut) = state.shortcut else {
        return;
    };
    let axis = ui.input(|input| {
        if input.key_pressed(egui::Key::X) {
            Some(GridAxis::X)
        } else if input.key_pressed(egui::Key::Y) {
            Some(GridAxis::Y)
        } else if input.key_pressed(egui::Key::Z) {
            Some(GridAxis::Z)
        } else {
            None
        }
    });
    let Some(axis) = axis else {
        return;
    };
    let Some(object) = selected else {
        state.reset();
        return;
    };
    let direction = if ui.input(|input| input.modifiers.shift) {
        -1
    } else {
        1
    };
    let command = match shortcut {
        TransformShortcut::Move => return,
        TransformShortcut::Rotate => {
            let Some(pivot_world) = object_rotation_pivot(project, object) else {
                state.reset();
                return;
            };
            Command::RotateQuarterAround {
                object,
                axis,
                quarter_turns: direction as i8,
                pivot_world,
            }
        }
    };
    commands.push(PendingCommand {
        command,
        select_created: false,
    });
    state.reset();
}

fn update_rotate_preview(session: &mut RotateSession, pointer: egui::Pos2) {
    let parameter = rotation_ring_parameter(session.center, &session.camera, session.axis, pointer);
    session.current_angle = normalize_signed_angle(parameter - session.start_parameter);
    session.quarter_turns = (session.current_angle / std::f32::consts::FRAC_PI_2)
        .round()
        .clamp(-2.0, 2.0) as i8;
}

fn draw_rotation_drag_indicator(painter: &egui::Painter, session: RotateSession) {
    let color = axis_color(session.axis);
    painter.add(egui::Shape::line(
        rotation_ring_points(session.center, &session.camera, session.axis),
        egui::Stroke::new(1.5, color),
    ));
    let start = rotation_ring_point(
        session.center,
        &session.camera,
        session.axis,
        session.start_parameter,
    );
    painter.circle_filled(start, 6.0, color);
    if session.current_angle.abs() < 0.02 {
        return;
    }
    let segment_count = ((session.current_angle.abs() / std::f32::consts::PI) * 40.0)
        .ceil()
        .max(2.0) as usize;
    let arc = (0..=segment_count)
        .map(|index| {
            let progress = index as f32 / segment_count as f32;
            rotation_ring_point(
                session.center,
                &session.camera,
                session.axis,
                session.start_parameter + session.current_angle * progress,
            )
        })
        .collect::<Vec<_>>();
    painter.add(egui::Shape::line(
        arc.clone(),
        egui::Stroke::new(5.5, color),
    ));
    let tip = *arc.last().expect("rotation arc has an endpoint");
    let forward = (tip - arc[arc.len() - 2]).normalized();
    let sideways = egui::vec2(-forward.y, forward.x);
    let arrow_base = tip - forward * 15.0;
    painter.add(egui::Shape::convex_polygon(
        vec![
            tip,
            arrow_base + sideways * 7.0,
            arrow_base - sideways * 7.0,
        ],
        color,
        egui::Stroke::NONE,
    ));
    painter.text(
        tip + sideways * 14.0,
        egui::Align2::CENTER_CENTER,
        format!("{:.0}°", session.current_angle.to_degrees()),
        egui::FontId::proportional(13.0),
        color,
    );
}

fn normalize_signed_angle(angle: f32) -> f32 {
    let normalized = angle.rem_euclid(std::f32::consts::TAU);
    if normalized > std::f32::consts::PI {
        normalized - std::f32::consts::TAU
    } else {
        normalized
    }
}

fn update_move_preview(
    project: &Project,
    session: &mut MoveSession,
    pointer: egui::Pos2,
    viewport_center: egui::Pos2,
    fallback_camera: Option<Camera>,
) {
    let Some(camera) = session.camera.or(fallback_camera) else {
        return;
    };
    session.camera = Some(camera);
    let axis_screen_direction = match session.constraint {
        MoveConstraint::Axis(axis) => object_gizmo_center(project, session.object)
            .and_then(|origin| projected_axis_direction(viewport_center, &camera, origin, axis)),
        _ => None,
    };
    let world_delta = constrained_world_delta(
        &camera,
        pointer - session.start_pointer,
        session.constraint,
        axis_screen_direction,
    );
    let local_delta = parent_world_rotation(project, session.object)
        .inverse()
        .apply(world_delta);
    session.preview_placement.translation =
        add_grid_positions(session.start_placement.translation, local_delta);
}

fn constrained_world_delta(
    camera: &Camera,
    screen_delta: egui::Vec2,
    constraint: MoveConstraint,
    axis_screen_direction: Option<egui::Vec2>,
) -> GridPosition {
    if camera.pixels_per_unit <= f32::EPSILON {
        return GridPosition::ZERO;
    }
    if let MoveConstraint::Axis(axis) = constraint {
        let screen_axis = axis_screen_direction.unwrap_or_else(|| {
            let projected = camera.orientation * axis_vector(axis);
            egui::vec2(projected.x, -projected.y)
        });
        if screen_axis.length_sq() <= f32::EPSILON {
            return GridPosition::ZERO;
        }
        let steps =
            (screen_delta.dot(screen_axis.normalized()) / camera.pixels_per_unit).round() as i32;
        return match axis {
            GridAxis::X => GridPosition::new(steps, 0, 0),
            GridAxis::Y => GridPosition::new(0, steps, 0),
            GridAxis::Z => GridPosition::new(0, 0, steps),
        };
    }

    let camera_delta = Vec3::new(screen_delta.x, -screen_delta.y, 0.0) / camera.pixels_per_unit;
    let mut world_delta = camera.orientation.conjugate() * camera_delta;
    if let MoveConstraint::PlaneExcluding(axis) = constraint {
        match axis {
            GridAxis::X => world_delta.x = 0.0,
            GridAxis::Y => world_delta.y = 0.0,
            GridAxis::Z => world_delta.z = 0.0,
        }
    }
    GridPosition::new(
        world_delta.x.round() as i32,
        world_delta.y.round() as i32,
        world_delta.z.round() as i32,
    )
}

fn parent_world_rotation(project: &Project, object: ObjectRef) -> GridRotation {
    let mut parent = object_parent_group(project, object);
    let mut rotation = GridRotation::IDENTITY;
    while let Some(parent_id) = parent {
        let Some(group) = project.groups.get(&parent_id) else {
            break;
        };
        rotation = group.placement.rotation.compose(rotation);
        parent = group.parent_group_id;
    }
    rotation
}

const fn move_constraint_label(constraint: MoveConstraint) -> &'static str {
    match constraint {
        MoveConstraint::ViewPlane => "ビュー平面",
        MoveConstraint::Axis(GridAxis::X) => "X軸",
        MoveConstraint::Axis(GridAxis::Y) => "Y軸",
        MoveConstraint::Axis(GridAxis::Z) => "Z軸",
        MoveConstraint::PlaneExcluding(GridAxis::X) => "YZ平面 / Xを除外",
        MoveConstraint::PlaneExcluding(GridAxis::Y) => "XZ平面 / Yを除外",
        MoveConstraint::PlaneExcluding(GridAxis::Z) => "XY平面 / Zを除外",
    }
}

fn placement_for_object(project: &Project, object: ObjectRef) -> Option<Placement> {
    match object {
        ObjectRef::Group(id) => project.groups.get(&id).map(|group| group.placement),
        ObjectRef::Shape(id) => project.shapes.get(&id).map(|shape| shape.placement),
        ObjectRef::Piece(id) => project.pieces.get(&id).map(|piece| piece.placement),
    }
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

#[derive(Clone, Copy)]
struct Camera {
    orientation: Quat,
    target: Vec3,
    pixels_per_unit: f32,
    pan: egui::Vec2,
    perspective: bool,
    focal_distance: f32,
}

#[derive(Clone, Copy)]
struct CameraFrame {
    target: Vec3,
    span: f32,
}

struct RenderFace {
    object: ObjectRef,
    depth: f32,
    points: Vec<egui::Pos2>,
    color: egui::Color32,
}

#[derive(Clone, Copy)]
struct RenderOptions {
    selected: Option<ObjectRef>,
    orientation: Quat,
    zoom: f32,
    pan: egui::Vec2,
    perspective: bool,
    camera_frame: Option<CameraFrame>,
}

struct RenderOutput {
    faces: Vec<RenderFace>,
    camera: Option<Camera>,
    camera_frame: Option<CameraFrame>,
}

fn render_voxels(
    painter: &egui::Painter,
    rect: egui::Rect,
    project: &Project,
    options: RenderOptions,
) -> RenderOutput {
    let visible_objects = project
        .pieces
        .values()
        .filter(|piece| piece.visible && group_chain_is_visible(project, piece.parent_group_id))
        .map(|piece| {
            (
                ObjectRef::Piece(piece.id),
                piece.parent_group_id,
                piece.placement,
                &piece.beads,
            )
        })
        .chain(
            project
                .shapes
                .values()
                .filter(|shape| {
                    shape.visible && group_chain_is_visible(project, shape.parent_group_id)
                })
                .map(|shape| {
                    (
                        ObjectRef::Shape(shape.id),
                        shape.parent_group_id,
                        shape.placement,
                        &shape.beads,
                    )
                }),
        )
        .collect::<Vec<_>>();
    let occupied = visible_objects
        .iter()
        .flat_map(|(_, parent_group_id, placement, beads)| {
            beads.keys().map(|position| {
                transformed_world_position(project, *parent_group_id, *placement, *position)
            })
        })
        .collect::<BTreeSet<_>>();
    if occupied.is_empty() {
        return RenderOutput {
            faces: Vec::new(),
            camera: None,
            camera_frame: options.camera_frame,
        };
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
    let fitted_frame = CameraFrame {
        target: (min + max) * 0.5,
        span: (max - min + Vec3::ONE).max_element().max(1.0),
    };
    let frame = options.camera_frame.unwrap_or(fitted_frame);
    let camera = Camera {
        orientation: options.orientation,
        target: frame.target,
        pixels_per_unit: rect.width().min(rect.height()) * 0.65 / frame.span * options.zoom,
        pan: options.pan,
        perspective: options.perspective,
        focal_distance: frame.span * 4.0,
    };
    let mut faces = Vec::new();
    for (object, parent_group_id, placement, beads) in visible_objects {
        for (position, color) in beads {
            append_faces(
                &mut faces,
                rect.center(),
                object,
                transformed_world_position(project, parent_group_id, placement, *position),
                *color,
                &occupied,
                &camera,
            );
        }
    }
    faces.sort_by(|a, b| a.depth.total_cmp(&b.depth));
    for face in &faces {
        painter.add(egui::Shape::convex_polygon(
            face.points.clone(),
            face.color,
            egui::Stroke::NONE,
        ));
    }
    let selected_faces = faces
        .iter()
        .filter(|face| object_is_selected(project, face.object, options.selected))
        .collect::<Vec<_>>();
    for edge in boundary_edges(&selected_faces) {
        painter.line_segment(
            edge,
            egui::Stroke::new(2.5, egui::Color32::from_rgb(255, 132, 18)),
        );
    }
    RenderOutput {
        faces,
        camera: Some(camera),
        camera_frame: Some(frame),
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ScreenEdgeKey {
    start: (i32, i32),
    end: (i32, i32),
}

fn boundary_edges(faces: &[&RenderFace]) -> Vec<[egui::Pos2; 2]> {
    let mut edges = BTreeMap::<ScreenEdgeKey, ([egui::Pos2; 2], usize)>::new();
    for face in faces {
        for index in 0..face.points.len() {
            let start = face.points[index];
            let end = face.points[(index + 1) % face.points.len()];
            let key = screen_edge_key(start, end);
            let entry = edges.entry(key).or_insert(([start, end], 0));
            entry.1 += 1;
        }
    }
    edges
        .into_values()
        .filter_map(|(edge, count)| (count == 1).then_some(edge))
        .collect()
}

fn screen_edge_key(start: egui::Pos2, end: egui::Pos2) -> ScreenEdgeKey {
    let start = quantized_screen_point(start);
    let end = quantized_screen_point(end);
    if start <= end {
        ScreenEdgeKey { start, end }
    } else {
        ScreenEdgeKey {
            start: end,
            end: start,
        }
    }
}

fn quantized_screen_point(point: egui::Pos2) -> (i32, i32) {
    (
        (point.x * 1000.0).round() as i32,
        (point.y * 1000.0).round() as i32,
    )
}

fn transformed_world_position(
    project: &Project,
    mut parent_group_id: ObjectId,
    object_placement: Placement,
    position: GridPosition,
) -> GridPosition {
    let mut world = add_grid_positions(
        object_placement.rotation.apply(position),
        object_placement.translation,
    );
    loop {
        let Some(group) = project.groups.get(&parent_group_id) else {
            return world;
        };
        world = add_grid_positions(
            group.placement.rotation.apply(world),
            group.placement.translation,
        );
        let Some(parent) = group.parent_group_id else {
            return world;
        };
        parent_group_id = parent;
    }
}

const fn add_grid_positions(left: GridPosition, right: GridPosition) -> GridPosition {
    GridPosition::new(left.x + right.x, left.y + right.y, left.z + right.z)
}

fn object_is_selected(project: &Project, object: ObjectRef, selected: Option<ObjectRef>) -> bool {
    match selected {
        Some(selected) if selected == object => true,
        Some(ObjectRef::Group(group_id)) => {
            let mut parent = object_parent_group(project, object);
            while let Some(parent_id) = parent {
                if parent_id == group_id {
                    return true;
                }
                parent = project
                    .groups
                    .get(&parent_id)
                    .and_then(|group| group.parent_group_id);
            }
            false
        }
        _ => false,
    }
}

fn pick_rendered_object(faces: &[RenderFace], pointer: egui::Pos2) -> Option<ObjectRef> {
    faces
        .iter()
        .rev()
        .find(|face| point_in_convex_polygon(pointer, &face.points))
        .map(|face| face.object)
}

fn point_in_convex_polygon(point: egui::Pos2, polygon: &[egui::Pos2]) -> bool {
    let mut sign = 0.0_f32;
    for index in 0..polygon.len() {
        let start = polygon[index];
        let end = polygon[(index + 1) % polygon.len()];
        let cross =
            (end.x - start.x) * (point.y - start.y) - (end.y - start.y) * (point.x - start.x);
        if cross.abs() <= f32::EPSILON {
            continue;
        }
        if sign == 0.0 {
            sign = cross.signum();
        } else if sign != cross.signum() {
            return false;
        }
    }
    true
}

fn group_chain_is_visible(project: &Project, mut group_id: ObjectId) -> bool {
    loop {
        let Some(group) = project.groups.get(&group_id) else {
            return false;
        };
        if !group.visible {
            return false;
        }
        let Some(parent) = group.parent_group_id else {
            return true;
        };
        group_id = parent;
    }
}

fn append_faces(
    output: &mut Vec<RenderFace>,
    center: egui::Pos2,
    object: ObjectRef,
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
            object,
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

#[cfg(test)]
mod presentation_tests {
    use super::*;

    #[test]
    fn only_groups_are_valid_reparent_targets_and_cycles_are_rejected() {
        let mut project = Project::new("tree");
        let root = project.root_group_id();
        let parent = project.create_group(root, "parent").unwrap();
        let child = project.create_group(parent, "child").unwrap();
        let piece = project
            .create_piece(root, "piece", Plane::Xy { z: 0 })
            .unwrap();

        assert!(can_reparent(&project, ObjectRef::Piece(piece), parent));
        assert!(!can_reparent(&project, ObjectRef::Piece(piece), root));
        assert!(!can_reparent(&project, ObjectRef::Group(parent), child));
        assert!(!can_reparent(&project, ObjectRef::Group(root), child));
        assert!(can_move_after(
            &project,
            ObjectRef::Piece(piece),
            ObjectRef::Group(parent)
        ));
        assert!(!can_move_after(
            &project,
            ObjectRef::Group(parent),
            ObjectRef::Group(child)
        ));
        assert!(!can_move_after(
            &project,
            ObjectRef::Group(root),
            ObjectRef::Piece(piece)
        ));
    }

    #[test]
    fn hidden_ancestor_hides_descendant_pieces() {
        let mut project = Project::new("visibility");
        let root = project.root_group_id();
        let group = project.create_group(root, "part").unwrap();
        let piece = project
            .create_piece(group, "piece", Plane::Xy { z: 0 })
            .unwrap();

        assert!(group_chain_is_visible(
            &project,
            project.pieces[&piece].parent_group_id
        ));
        project
            .set_visibility(ObjectRef::Group(group), false)
            .unwrap();
        assert!(!group_chain_is_visible(
            &project,
            project.pieces[&piece].parent_group_id
        ));
    }

    #[test]
    fn group_selection_includes_descendant_objects() {
        let mut project = Project::new("selection");
        let root = project.root_group_id();
        let group = project.create_group(root, "group").unwrap();
        let nested = project.create_group(group, "nested").unwrap();
        let inside = project
            .create_piece(nested, "inside", Plane::Xy { z: 0 })
            .unwrap();
        let outside = project
            .create_piece(root, "outside", Plane::Xy { z: 0 })
            .unwrap();

        assert!(object_is_selected(
            &project,
            ObjectRef::Piece(inside),
            Some(ObjectRef::Group(group))
        ));
        assert!(!object_is_selected(
            &project,
            ObjectRef::Piece(outside),
            Some(ObjectRef::Group(group))
        ));
    }

    #[test]
    fn picking_uses_the_frontmost_face_containing_the_pointer() {
        let square = vec![
            egui::pos2(0.0, 0.0),
            egui::pos2(10.0, 0.0),
            egui::pos2(10.0, 10.0),
            egui::pos2(0.0, 10.0),
        ];
        let faces = vec![
            RenderFace {
                object: ObjectRef::Piece(1),
                depth: 0.0,
                points: square.clone(),
                color: egui::Color32::WHITE,
            },
            RenderFace {
                object: ObjectRef::Piece(2),
                depth: 1.0,
                points: square,
                color: egui::Color32::WHITE,
            },
        ];

        assert_eq!(
            pick_rendered_object(&faces, egui::pos2(5.0, 5.0)),
            Some(ObjectRef::Piece(2))
        );
        assert_eq!(pick_rendered_object(&faces, egui::pos2(20.0, 20.0)), None);
    }

    #[test]
    fn selection_outline_omits_edges_shared_by_neighboring_faces() {
        let left = RenderFace {
            object: ObjectRef::Piece(1),
            depth: 0.0,
            points: vec![
                egui::pos2(0.0, 0.0),
                egui::pos2(10.0, 0.0),
                egui::pos2(10.0, 10.0),
                egui::pos2(0.0, 10.0),
            ],
            color: egui::Color32::WHITE,
        };
        let right = RenderFace {
            object: ObjectRef::Piece(1),
            depth: 0.0,
            points: vec![
                egui::pos2(10.0, 0.0),
                egui::pos2(20.0, 0.0),
                egui::pos2(20.0, 10.0),
                egui::pos2(10.0, 10.0),
            ],
            color: egui::Color32::WHITE,
        };

        let edges = boundary_edges(&[&left, &right]);
        assert_eq!(edges.len(), 6);
        assert!(!edges.iter().any(|edge| {
            screen_edge_key(edge[0], edge[1])
                == screen_edge_key(egui::pos2(10.0, 0.0), egui::pos2(10.0, 10.0))
        }));
    }

    #[test]
    fn object_and_group_translations_are_composed_for_rendering() {
        let mut project = Project::new("placement");
        let root = project.root_group_id();
        let group = project.create_group(root, "group").unwrap();
        project.groups.get_mut(&root).unwrap().placement.translation = GridPosition::new(1, 0, 0);
        project
            .groups
            .get_mut(&group)
            .unwrap()
            .placement
            .translation = GridPosition::new(0, 2, 0);

        assert_eq!(
            transformed_world_position(
                &project,
                group,
                Placement {
                    translation: GridPosition::new(0, 0, 3),
                    ..Placement::default()
                },
                GridPosition::new(4, 5, 6)
            ),
            GridPosition::new(5, 7, 9)
        );
    }

    #[test]
    fn object_and_group_rotations_are_composed_for_rendering() {
        let mut project = Project::new("rotation");
        let root = project.root_group_id();
        let group = project.create_group(root, "group").unwrap();
        project.groups.get_mut(&group).unwrap().placement.rotation =
            Placement::default().rotation.rotate_quarter(GridAxis::Z, 1);
        let placement = Placement {
            translation: GridPosition::new(1, 0, 0),
            ..Placement::default()
        };

        assert_eq!(
            transformed_world_position(&project, group, placement, GridPosition::new(1, 0, 0)),
            GridPosition::new(0, 2, 0)
        );
    }

    #[test]
    fn shifted_axis_constraints_exclude_that_world_axis() {
        let camera = Camera {
            orientation: Quat::IDENTITY,
            target: Vec3::ZERO,
            pixels_per_unit: 10.0,
            pan: egui::Vec2::ZERO,
            perspective: false,
            focal_distance: 10.0,
        };

        assert_eq!(
            constrained_world_delta(
                &camera,
                egui::vec2(20.0, -30.0),
                MoveConstraint::PlaneExcluding(GridAxis::X),
                None,
            ),
            GridPosition::new(0, 3, 0)
        );
        assert_eq!(
            constrained_world_delta(
                &camera,
                egui::vec2(20.0, -30.0),
                MoveConstraint::PlaneExcluding(GridAxis::Y),
                None,
            ),
            GridPosition::new(2, 0, 0)
        );
    }

    #[test]
    fn world_move_is_converted_into_parent_local_coordinates() {
        let mut project = Project::new("move");
        let root = project.root_group_id();
        let group = project.create_group(root, "rotated").unwrap();
        let piece = project
            .create_piece(group, "piece", Plane::Xy { z: 0 })
            .unwrap();
        project.groups.get_mut(&group).unwrap().placement.rotation =
            GridRotation::IDENTITY.rotate_quarter(GridAxis::Z, 1);

        assert_eq!(
            parent_world_rotation(&project, ObjectRef::Piece(piece))
                .inverse()
                .apply(GridPosition::new(1, 0, 0)),
            GridPosition::new(0, -1, 0)
        );
    }

    #[test]
    fn gizmo_center_uses_the_selected_voxel_bounds() {
        let mut project = Project::new("gizmo center");
        let root = project.root_group_id();
        let piece = project
            .create_piece(root, "piece", Plane::Xy { z: 0 })
            .unwrap();
        project
            .add_bead(
                VoxelObjectRef::Piece(piece),
                Bead {
                    position: GridPosition::new(0, 0, 0),
                    color: Color::RED,
                },
            )
            .unwrap();
        project
            .add_bead(
                VoxelObjectRef::Piece(piece),
                Bead {
                    position: GridPosition::new(4, 2, 0),
                    color: Color::RED,
                },
            )
            .unwrap();
        project
            .set_placement(
                ObjectRef::Piece(piece),
                Placement {
                    translation: GridPosition::new(10, -3, 1),
                    ..Placement::default()
                },
            )
            .unwrap();

        assert_eq!(
            object_gizmo_center(&project, ObjectRef::Piece(piece)),
            Some(Vec3::new(12.0, -2.0, 1.0))
        );
    }

    #[test]
    fn rotation_drag_snaps_preview_to_quarter_turns() {
        let camera = Camera {
            orientation: Quat::IDENTITY,
            target: Vec3::ZERO,
            pixels_per_unit: 10.0,
            pan: egui::Vec2::ZERO,
            perspective: false,
            focal_distance: 10.0,
        };
        let mut session = RotateSession {
            object: ObjectRef::Piece(1),
            axis: GridAxis::Z,
            pivot_world: GridPosition::ZERO,
            center: egui::Pos2::ZERO,
            start_parameter: 0.0,
            current_angle: 0.0,
            quarter_turns: 0,
            camera,
        };

        update_rotate_preview(&mut session, egui::pos2(0.0, -ROTATE_GIZMO_RADIUS));

        assert_eq!(session.quarter_turns, 1);
        assert_eq!(session.current_angle, std::f32::consts::FRAC_PI_2);
    }

    #[test]
    fn changing_orbit_target_preserves_its_screen_position() {
        let mut project = Project::new("orbit target");
        let root = project.root_group_id();
        let piece = project
            .create_piece(root, "piece", Plane::Xy { z: 0 })
            .unwrap();
        project
            .add_bead(
                VoxelObjectRef::Piece(piece),
                Bead {
                    position: GridPosition::new(4, 2, 0),
                    color: Color::RED,
                },
            )
            .unwrap();
        let camera = Camera {
            orientation: Quat::from_rotation_x(-0.4) * Quat::from_rotation_z(0.6),
            target: Vec3::ZERO,
            pixels_per_unit: 20.0,
            pan: egui::vec2(12.0, -8.0),
            perspective: true,
            focal_distance: 20.0,
        };
        let mut editor = AssemblyEditorState {
            last_camera: Some(camera),
            camera_frame: Some(CameraFrame {
                target: camera.target,
                span: 8.0,
            }),
            ..AssemblyEditorState::default()
        };
        let viewport_center = egui::pos2(300.0, 200.0);
        let target = object_gizmo_center(&project, ObjectRef::Piece(piece)).unwrap();
        let before = world_to_screen(viewport_center, target, &camera);
        let mut pan = camera.pan;

        retarget_orbit_center(
            &project,
            ObjectRef::Piece(piece),
            viewport_center,
            &mut editor,
            &mut pan,
        );

        let after = world_to_screen(viewport_center, target, &editor.last_camera.unwrap());
        assert!(before.distance(after) < 0.001);
        assert_eq!(editor.camera_frame.unwrap().target, target);
    }
}

pub fn run<R>(repository: R) -> eframe::Result
where
    R: ProjectRepository + 'static,
    R::Error: Debug,
{
    eframe::run_native(
        "ハコニワ",
        eframe::NativeOptions::default(),
        Box::new(move |cc| Ok(Box::new(HakoniwaApp::new(&cc.egui_ctx, repository)))),
    )
}
