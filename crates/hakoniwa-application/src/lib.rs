//! Application commands and undo/redo history.

use hakoniwa_domain::{Bead, DomainError, GridPosition, ObjectId, Plane, Project};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    AddBead {
        shape_id: ObjectId,
        bead: Bead,
    },
    ConvertPlaneToPiece {
        shape_id: ObjectId,
        plane: Plane,
        name: String,
    },
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

    pub fn execute(&mut self, command: Command) -> Result<(), DomainError> {
        let before = self.project.clone();
        match command {
            Command::AddBead { shape_id, bead } => self.project.add_shape_bead(shape_id, bead)?,
            Command::ConvertPlaneToPiece {
                shape_id,
                plane,
                name,
            } => {
                self.project
                    .convert_shape_plane_to_piece(shape_id, plane, name)?;
            }
        }
        self.undo.push(before);
        self.redo.clear();
        Ok(())
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
        color: hakoniwa_domain::Color,
    ) -> Result<(), DomainError> {
        self.execute(Command::AddBead {
            shape_id,
            bead: Bead { position, color },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hakoniwa_domain::Color;

    #[test]
    fn undo_and_redo_restore_project_state() {
        let mut project = Project::new("hammer");
        let shape = project.create_shape("head");
        let mut editor = Editor::new(project);
        editor
            .place_bead(shape, GridPosition::new(0, 0, 0), Color::RED)
            .unwrap();
        assert_eq!(editor.project().bead_count(), 1);
        assert!(editor.undo());
        assert_eq!(editor.project().bead_count(), 0);
        assert!(editor.redo());
        assert_eq!(editor.project().bead_count(), 1);
    }
}
