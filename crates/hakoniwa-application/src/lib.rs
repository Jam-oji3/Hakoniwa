//! Application commands and undo/redo history.

use std::collections::BTreeSet;

use hakoniwa_domain::{
    Bead, Color, DomainError, GridPosition, ObjectId, ObjectRef, OrthogonalOrientation, Placement,
    Plane, Project, VoxelObjectRef,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    CreateGroup {
        parent_group_id: ObjectId,
        name: String,
    },
    CreateShape {
        parent_group_id: ObjectId,
        name: String,
    },
    CreatePiece {
        parent_group_id: ObjectId,
        name: String,
        plane: Plane,
    },
    DeleteObject {
        object: ObjectRef,
    },
    AddBead {
        target: VoxelObjectRef,
        bead: Bead,
    },
    RemoveBead {
        target: VoxelObjectRef,
        position: GridPosition,
    },
    RecolorBead {
        target: VoxelObjectRef,
        position: GridPosition,
        color: Color,
    },
    MoveBead {
        target: VoxelObjectRef,
        from: GridPosition,
        to: GridPosition,
    },
    SetPlacement {
        object: ObjectRef,
        placement: Placement,
    },
    SetOrientation {
        object: ObjectRef,
        orientation: OrthogonalOrientation,
    },
    SetVisibility {
        object: ObjectRef,
        visible: bool,
    },
    ReparentObject {
        object: ObjectRef,
        new_parent_group_id: ObjectId,
    },
    ConvertPlaneToPiece {
        shape_id: ObjectId,
        plane: Plane,
        name: String,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ChangeSet {
    pub changed_objects: BTreeSet<ObjectRef>,
    pub dirty_piece_ids: BTreeSet<ObjectId>,
    pub nearby_positions: BTreeSet<GridPosition>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CommandResult {
    pub changes: ChangeSet,
    pub created_object: Option<ObjectRef>,
}

#[derive(Clone, Debug)]
pub struct Editor {
    project: Project,
    undo: Vec<Project>,
    redo: Vec<Project>,
}

impl Editor {
    #[must_use]
    pub fn new(project: Project) -> Self {
        Self {
            project,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    #[must_use]
    pub fn project(&self) -> &Project {
        &self.project
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn execute(&mut self, command: Command) -> Result<CommandResult, DomainError> {
        let before = self.project.clone();
        let mut candidate = before.clone();
        let result = apply_command(&mut candidate, command)?;
        candidate.validate()?;
        self.project = candidate;
        self.undo.push(before);
        self.redo.clear();
        Ok(result)
    }

    pub fn undo(&mut self) -> bool {
        if let Some(previous) = self.undo.pop() {
            self.redo
                .push(std::mem::replace(&mut self.project, previous));
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.project, next));
            true
        } else {
            false
        }
    }

    pub fn place_bead(
        &mut self,
        shape_id: ObjectId,
        position: GridPosition,
        color: Color,
    ) -> Result<CommandResult, DomainError> {
        self.execute(Command::AddBead {
            target: VoxelObjectRef::Shape(shape_id),
            bead: Bead { position, color },
        })
    }
}

fn apply_command(project: &mut Project, command: Command) -> Result<CommandResult, DomainError> {
    let mut result = CommandResult::default();
    match command {
        Command::CreateGroup {
            parent_group_id,
            name,
        } => {
            let id = project.create_group(parent_group_id, name)?;
            record_object(&mut result.changes, ObjectRef::Group(id));
            result.created_object = Some(ObjectRef::Group(id));
        }
        Command::CreateShape {
            parent_group_id,
            name,
        } => {
            let id = project.create_shape_in_group(parent_group_id, name)?;
            record_object(&mut result.changes, ObjectRef::Shape(id));
            result.created_object = Some(ObjectRef::Shape(id));
        }
        Command::CreatePiece {
            parent_group_id,
            name,
            plane,
        } => {
            let id = project.create_piece(parent_group_id, name, plane)?;
            record_object(&mut result.changes, ObjectRef::Piece(id));
            result.created_object = Some(ObjectRef::Piece(id));
        }
        Command::DeleteObject { object } => {
            record_affected_pieces(project, object, &mut result.changes);
            project.delete_object(object)?;
            record_object(&mut result.changes, object);
        }
        Command::AddBead { target, bead } => {
            project.add_bead(target, bead)?;
            record_voxel_change(&mut result.changes, target, bead.position);
        }
        Command::RemoveBead { target, position } => {
            project.remove_bead(target, position)?;
            record_voxel_change(&mut result.changes, target, position);
        }
        Command::RecolorBead {
            target,
            position,
            color,
        } => {
            project.recolor_bead(target, position, color)?;
            record_voxel_change(&mut result.changes, target, position);
        }
        Command::MoveBead { target, from, to } => {
            project.move_bead(target, from, to)?;
            record_voxel_change(&mut result.changes, target, from);
            result.changes.nearby_positions.insert(to);
        }
        Command::SetPlacement { object, placement } => {
            record_affected_pieces(project, object, &mut result.changes);
            project.set_placement(object, placement)?;
            record_object(&mut result.changes, object);
        }
        Command::SetOrientation {
            object,
            orientation,
        } => {
            let mut placement = object_placement(project, object)?;
            placement.orientation = orientation;
            record_affected_pieces(project, object, &mut result.changes);
            project.set_placement(object, placement)?;
            record_object(&mut result.changes, object);
        }
        Command::SetVisibility { object, visible } => {
            record_affected_pieces(project, object, &mut result.changes);
            project.set_visibility(object, visible)?;
            record_object(&mut result.changes, object);
        }
        Command::ReparentObject {
            object,
            new_parent_group_id,
        } => {
            record_affected_pieces(project, object, &mut result.changes);
            project.reparent_object(object, new_parent_group_id)?;
            record_object(&mut result.changes, object);
        }
        Command::ConvertPlaneToPiece {
            shape_id,
            plane,
            name,
        } => {
            let piece_id = project.convert_shape_plane_to_piece(shape_id, plane, name)?;
            record_object(&mut result.changes, ObjectRef::Shape(shape_id));
            record_object(&mut result.changes, ObjectRef::Piece(piece_id));
            for position in project.pieces[&piece_id].beads.keys() {
                result.changes.nearby_positions.insert(*position);
            }
            result.created_object = Some(ObjectRef::Piece(piece_id));
        }
    }
    Ok(result)
}

fn object_placement(project: &Project, object: ObjectRef) -> Result<Placement, DomainError> {
    match object {
        ObjectRef::Group(id) => project
            .groups
            .get(&id)
            .map(|group| group.placement)
            .ok_or(DomainError::GroupNotFound(id)),
        ObjectRef::Shape(id) => project
            .shapes
            .get(&id)
            .map(|shape| shape.placement)
            .ok_or(DomainError::ShapeNotFound(id)),
        ObjectRef::Piece(id) => project
            .pieces
            .get(&id)
            .map(|piece| piece.placement)
            .ok_or(DomainError::PieceNotFound(id)),
    }
}

fn record_voxel_change(changes: &mut ChangeSet, target: VoxelObjectRef, position: GridPosition) {
    let object = match target {
        VoxelObjectRef::Shape(id) => ObjectRef::Shape(id),
        VoxelObjectRef::Piece(id) => ObjectRef::Piece(id),
    };
    record_object(changes, object);
    changes.nearby_positions.insert(position);
}

fn record_object(changes: &mut ChangeSet, object: ObjectRef) {
    changes.changed_objects.insert(object);
    if let ObjectRef::Piece(id) = object {
        changes.dirty_piece_ids.insert(id);
    }
}

fn record_affected_pieces(project: &Project, object: ObjectRef, changes: &mut ChangeSet) {
    match object {
        ObjectRef::Piece(id) => {
            changes.dirty_piece_ids.insert(id);
        }
        ObjectRef::Shape(_) => {}
        ObjectRef::Group(group_id) => {
            for piece in project.pieces.values() {
                if is_group_descendant(project, piece.parent_group_id, group_id) {
                    changes.dirty_piece_ids.insert(piece.id);
                }
            }
        }
    }
}

fn is_group_descendant(project: &Project, mut group_id: ObjectId, ancestor_id: ObjectId) -> bool {
    loop {
        if group_id == ancestor_id {
            return true;
        }
        let Some(parent) = project
            .groups
            .get(&group_id)
            .and_then(|group| group.parent_group_id)
        else {
            return false;
        };
        group_id = parent;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece_editor() -> (Editor, ObjectId) {
        let mut project = Project::new("hammer");
        let piece = project
            .create_piece(project.root_group_id(), "head", Plane::Xy { z: 0 })
            .unwrap();
        (Editor::new(project), piece)
    }

    #[test]
    fn undo_and_redo_restore_exact_project_state() {
        let (mut editor, piece) = piece_editor();
        let before = editor.project().clone();
        let result = editor
            .execute(Command::AddBead {
                target: VoxelObjectRef::Piece(piece),
                bead: Bead {
                    position: GridPosition::ZERO,
                    color: Color::RED,
                },
            })
            .unwrap();
        let after = editor.project().clone();
        assert_eq!(result.changes.dirty_piece_ids, BTreeSet::from([piece]));
        assert!(editor.undo());
        assert_eq!(editor.project(), &before);
        assert!(editor.redo());
        assert_eq!(editor.project(), &after);
    }

    #[test]
    fn representative_edits_round_trip_through_history() {
        let (mut editor, piece) = piece_editor();
        let root = editor.project().root_group_id();
        let baseline = editor.project().clone();
        let position = GridPosition::ZERO;
        let moved = GridPosition::new(1, 0, 0);

        editor
            .execute(Command::AddBead {
                target: VoxelObjectRef::Piece(piece),
                bead: Bead {
                    position,
                    color: Color::RED,
                },
            })
            .unwrap();
        editor
            .execute(Command::RecolorBead {
                target: VoxelObjectRef::Piece(piece),
                position,
                color: Color::BROWN,
            })
            .unwrap();
        editor
            .execute(Command::MoveBead {
                target: VoxelObjectRef::Piece(piece),
                from: position,
                to: moved,
            })
            .unwrap();
        editor
            .execute(Command::SetPlacement {
                object: ObjectRef::Piece(piece),
                placement: Placement {
                    translation: GridPosition::new(4, 5, 6),
                    orientation: OrthogonalOrientation::Yz,
                },
            })
            .unwrap();
        let group = editor
            .execute(Command::CreateGroup {
                parent_group_id: root,
                name: "subassembly".to_owned(),
            })
            .unwrap()
            .created_object
            .unwrap();
        editor
            .execute(Command::ReparentObject {
                object: ObjectRef::Piece(piece),
                new_parent_group_id: group.id(),
            })
            .unwrap();
        editor
            .execute(Command::RemoveBead {
                target: VoxelObjectRef::Piece(piece),
                position: moved,
            })
            .unwrap();
        editor
            .execute(Command::DeleteObject {
                object: ObjectRef::Piece(piece),
            })
            .unwrap();
        let final_state = editor.project().clone();

        for _ in 0..8 {
            assert!(editor.undo());
        }
        assert_eq!(editor.project(), &baseline);
        for _ in 0..8 {
            assert!(editor.redo());
        }
        assert_eq!(editor.project(), &final_state);
    }

    #[test]
    fn piece_conversion_round_trips_with_the_same_id() {
        let mut project = Project::new("shape-first");
        let shape = project.create_shape("head");
        project
            .add_shape_bead(
                shape,
                Bead {
                    position: GridPosition::ZERO,
                    color: Color::RED,
                },
            )
            .unwrap();
        let mut editor = Editor::new(project);
        let before = editor.project().clone();
        let result = editor
            .execute(Command::ConvertPlaneToPiece {
                shape_id: shape,
                plane: Plane::Xy { z: 0 },
                name: "head piece".to_owned(),
            })
            .unwrap();
        let created = result.created_object;
        let after = editor.project().clone();
        assert!(editor.undo());
        assert_eq!(editor.project(), &before);
        assert!(editor.redo());
        assert_eq!(editor.project(), &after);
        assert_eq!(
            created,
            Some(ObjectRef::Piece(
                after.pieces.keys().next().copied().unwrap()
            ))
        );
    }

    #[test]
    fn failed_command_changes_neither_state_nor_history() {
        let (mut editor, piece) = piece_editor();
        let before = editor.project().clone();
        let error = editor.execute(Command::AddBead {
            target: VoxelObjectRef::Piece(piece),
            bead: Bead {
                position: GridPosition::new(0, 0, 1),
                color: Color::RED,
            },
        });
        assert!(matches!(error, Err(DomainError::PieceIsNotPlanar { .. })));
        assert_eq!(editor.project(), &before);
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    #[test]
    fn new_command_after_undo_discards_redo_history() {
        let (mut editor, piece) = piece_editor();
        editor
            .execute(Command::AddBead {
                target: VoxelObjectRef::Piece(piece),
                bead: Bead {
                    position: GridPosition::ZERO,
                    color: Color::RED,
                },
            })
            .unwrap();
        assert!(editor.undo());
        assert!(editor.can_redo());
        editor
            .execute(Command::AddBead {
                target: VoxelObjectRef::Piece(piece),
                bead: Bead {
                    position: GridPosition::new(1, 0, 0),
                    color: Color::BROWN,
                },
            })
            .unwrap();
        assert!(!editor.can_redo());
    }

    #[test]
    fn identical_command_sequences_reconstruct_identical_projects() {
        let build = || {
            let project = Project::new("repeatable");
            let root = project.root_group_id();
            let mut editor = Editor::new(project);
            let piece = editor
                .execute(Command::CreatePiece {
                    parent_group_id: root,
                    name: "piece".to_owned(),
                    plane: Plane::Xy { z: 0 },
                })
                .unwrap()
                .created_object
                .unwrap();
            editor
                .execute(Command::AddBead {
                    target: VoxelObjectRef::Piece(piece.id()),
                    bead: Bead {
                        position: GridPosition::ZERO,
                        color: Color::RED,
                    },
                })
                .unwrap();
            editor.project().clone()
        };
        assert_eq!(build(), build());
    }
}
