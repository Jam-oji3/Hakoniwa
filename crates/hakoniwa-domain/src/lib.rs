//! Core project model independent from UI, rendering, and file formats.

use std::collections::BTreeMap;

pub type ObjectId = u64;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct GridPosition {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl GridPosition {
    #[must_use]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Plane {
    Xy { z: i32 },
    Xz { y: i32 },
    Yz { x: i32 },
}

impl Plane {
    #[must_use]
    pub const fn contains(self, position: GridPosition) -> bool {
        match self {
            Self::Xy { z } => position.z == z,
            Self::Xz { y } => position.y == y,
            Self::Yz { x } => position.x == x,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Color(pub u8, pub u8, pub u8);

impl Color {
    pub const RED: Self = Self(220, 58, 52);
    pub const BROWN: Self = Self(117, 78, 47);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Bead {
    pub position: GridPosition,
    pub color: Color,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Shape {
    pub id: ObjectId,
    pub name: String,
    pub beads: BTreeMap<GridPosition, Color>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Piece {
    pub id: ObjectId,
    pub name: String,
    pub plane: Plane,
    pub beads: BTreeMap<GridPosition, Color>,
}

impl Piece {
    pub fn new(
        id: ObjectId,
        name: impl Into<String>,
        plane: Plane,
        beads: impl IntoIterator<Item = Bead>,
    ) -> Result<Self, DomainError> {
        let beads = beads
            .into_iter()
            .map(|bead| (bead.position, bead.color))
            .collect();
        let piece = Self {
            id,
            name: name.into(),
            plane,
            beads,
        };
        piece.validate()?;
        Ok(piece)
    }

    pub fn validate(&self) -> Result<(), DomainError> {
        if let Some(position) = self
            .beads
            .keys()
            .find(|position| !self.plane.contains(**position))
        {
            return Err(DomainError::PieceIsNotPlanar {
                piece_id: self.id,
                position: *position,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputReadiness {
    pub remaining_shape_ids: Vec<ObjectId>,
}

impl OutputReadiness {
    #[must_use]
    pub const fn is_ready(&self) -> bool {
        self.remaining_shape_ids.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Project {
    pub name: String,
    pub shapes: BTreeMap<ObjectId, Shape>,
    pub pieces: BTreeMap<ObjectId, Piece>,
    next_id: ObjectId,
}

impl Project {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            shapes: BTreeMap::new(),
            pieces: BTreeMap::new(),
            next_id: 1,
        }
    }

    pub fn create_shape(&mut self, name: impl Into<String>) -> ObjectId {
        let id = self.allocate_id();
        self.shapes.insert(
            id,
            Shape {
                id,
                name: name.into(),
                beads: BTreeMap::new(),
            },
        );
        id
    }

    pub fn add_shape_bead(&mut self, shape_id: ObjectId, bead: Bead) -> Result<(), DomainError> {
        let shape = self
            .shapes
            .get_mut(&shape_id)
            .ok_or(DomainError::ShapeNotFound(shape_id))?;
        shape.beads.insert(bead.position, bead.color);
        Ok(())
    }

    pub fn convert_shape_plane_to_piece(
        &mut self,
        shape_id: ObjectId,
        plane: Plane,
        name: impl Into<String>,
    ) -> Result<ObjectId, DomainError> {
        let position_colors = {
            let shape = self
                .shapes
                .get(&shape_id)
                .ok_or(DomainError::ShapeNotFound(shape_id))?;
            shape
                .beads
                .iter()
                .filter(|(position, _)| plane.contains(**position))
                .map(|(position, color)| (*position, *color))
                .collect::<BTreeMap<_, _>>()
        };
        if position_colors.is_empty() {
            return Err(DomainError::NoBeadsOnPlane { shape_id, plane });
        }
        let piece_id = self.allocate_id();
        let piece = Piece::new(
            piece_id,
            name,
            plane,
            position_colors.iter().map(|(position, color)| Bead {
                position: *position,
                color: *color,
            }),
        )?;
        let shape = self
            .shapes
            .get_mut(&shape_id)
            .ok_or(DomainError::ShapeNotFound(shape_id))?;
        for position in position_colors.keys() {
            shape.beads.remove(position);
        }
        self.pieces.insert(piece_id, piece);
        Ok(piece_id)
    }

    #[must_use]
    pub fn output_readiness(&self) -> OutputReadiness {
        OutputReadiness {
            remaining_shape_ids: self
                .shapes
                .values()
                .filter(|shape| !shape.beads.is_empty())
                .map(|shape| shape.id)
                .collect(),
        }
    }

    #[must_use]
    pub fn bead_count(&self) -> usize {
        self.shapes
            .values()
            .map(|shape| shape.beads.len())
            .sum::<usize>()
            + self
                .pieces
                .values()
                .map(|piece| piece.beads.len())
                .sum::<usize>()
    }

    fn allocate_id(&mut self) -> ObjectId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DomainError {
    ShapeNotFound(ObjectId),
    PieceIsNotPlanar {
        piece_id: ObjectId,
        position: GridPosition,
    },
    NoBeadsOnPlane {
        shape_id: ObjectId,
        plane: Plane,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_planar_piece() {
        let result = Piece::new(
            1,
            "bad",
            Plane::Xy { z: 0 },
            [Bead {
                position: GridPosition::new(0, 0, 1),
                color: Color::RED,
            }],
        );
        assert_eq!(
            result,
            Err(DomainError::PieceIsNotPlanar {
                piece_id: 1,
                position: GridPosition::new(0, 0, 1)
            })
        );
    }

    #[test]
    fn conversion_requires_a_plane_and_blocks_output_until_the_shape_is_empty() {
        let mut project = Project::new("hammer");
        let shape_id = project.create_shape("head");
        project
            .add_shape_bead(
                shape_id,
                Bead {
                    position: GridPosition::new(0, 0, 0),
                    color: Color::RED,
                },
            )
            .unwrap();
        project
            .add_shape_bead(
                shape_id,
                Bead {
                    position: GridPosition::new(0, 0, 1),
                    color: Color::RED,
                },
            )
            .unwrap();
        project
            .convert_shape_plane_to_piece(shape_id, Plane::Xy { z: 0 }, "head front")
            .unwrap();
        assert!(!project.output_readiness().is_ready());
        project
            .convert_shape_plane_to_piece(shape_id, Plane::Xy { z: 1 }, "head back")
            .unwrap();
        assert!(project.output_readiness().is_ready());
    }
}
