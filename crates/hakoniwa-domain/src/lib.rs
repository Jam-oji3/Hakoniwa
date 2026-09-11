//! Core project model independent from UI, rendering, and file formats.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub type ObjectId = u64;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct GridPosition {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Serialize for GridPosition {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("{},{},{}", self.x, self.y, self.z))
    }
}

impl<'de> Deserialize<'de> for GridPosition {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        let mut values = value.split(',').map(str::parse::<i32>);
        let (Some(Ok(x)), Some(Ok(y)), Some(Ok(z)), None) =
            (values.next(), values.next(), values.next(), values.next())
        else {
            return Err(serde::de::Error::custom("grid position must be x,y,z"));
        };
        Ok(Self { x, y, z })
    }
}

impl GridPosition {
    pub const ZERO: Self = Self::new(0, 0, 0);

    #[must_use]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct GridSize {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum OrthogonalOrientation {
    Xy,
    Xz,
    Yz,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum GridAxis {
    X,
    Y,
    Z,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct GridRotation {
    matrix: [[i8; 3]; 3],
}

impl GridRotation {
    pub const IDENTITY: Self = Self {
        matrix: [[1, 0, 0], [0, 1, 0], [0, 0, 1]],
    };

    #[must_use]
    pub const fn apply(self, position: GridPosition) -> GridPosition {
        let values = [position.x, position.y, position.z];
        GridPosition::new(
            self.matrix[0][0] as i32 * values[0]
                + self.matrix[0][1] as i32 * values[1]
                + self.matrix[0][2] as i32 * values[2],
            self.matrix[1][0] as i32 * values[0]
                + self.matrix[1][1] as i32 * values[1]
                + self.matrix[1][2] as i32 * values[2],
            self.matrix[2][0] as i32 * values[0]
                + self.matrix[2][1] as i32 * values[1]
                + self.matrix[2][2] as i32 * values[2],
        )
    }

    #[must_use]
    pub fn rotate_quarter(self, axis: GridAxis, quarter_turns: i8) -> Self {
        let turn = match axis {
            GridAxis::X => Self {
                matrix: [[1, 0, 0], [0, 0, -1], [0, 1, 0]],
            },
            GridAxis::Y => Self {
                matrix: [[0, 0, 1], [0, 1, 0], [-1, 0, 0]],
            },
            GridAxis::Z => Self {
                matrix: [[0, -1, 0], [1, 0, 0], [0, 0, 1]],
            },
        };
        let mut result = self;
        for _ in 0..quarter_turns.rem_euclid(4) {
            result = turn.compose(result);
        }
        result
    }

    #[must_use]
    pub fn is_valid(self) -> bool {
        let rows_are_unit = self
            .matrix
            .iter()
            .all(|row| row.iter().map(|value| value.unsigned_abs()).sum::<u8>() == 1);
        let columns_are_unit = (0..3).all(|column| {
            (0..3)
                .map(|row| self.matrix[row][column].unsigned_abs())
                .sum::<u8>()
                == 1
        });
        rows_are_unit && columns_are_unit && self.determinant() == 1
    }

    #[must_use]
    pub const fn compose(self, right: Self) -> Self {
        let mut matrix = [[0; 3]; 3];
        let mut row = 0;
        while row < 3 {
            let mut column = 0;
            while column < 3 {
                let mut index = 0;
                while index < 3 {
                    matrix[row][column] += self.matrix[row][index] * right.matrix[index][column];
                    index += 1;
                }
                column += 1;
            }
            row += 1;
        }
        Self { matrix }
    }

    #[must_use]
    pub const fn inverse(self) -> Self {
        Self {
            matrix: [
                [self.matrix[0][0], self.matrix[1][0], self.matrix[2][0]],
                [self.matrix[0][1], self.matrix[1][1], self.matrix[2][1]],
                [self.matrix[0][2], self.matrix[1][2], self.matrix[2][2]],
            ],
        }
    }

    const fn determinant(self) -> i8 {
        let m = self.matrix;
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    }
}

impl Default for GridRotation {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Placement {
    pub translation: GridPosition,
    pub orientation: OrthogonalOrientation,
    #[serde(default)]
    pub rotation: GridRotation,
}

impl Default for Placement {
    fn default() -> Self {
        Self {
            translation: GridPosition::ZERO,
            orientation: OrthogonalOrientation::Xy,
            rotation: GridRotation::IDENTITY,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
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

    #[must_use]
    pub const fn orientation(self) -> OrthogonalOrientation {
        match self {
            Self::Xy { .. } => OrthogonalOrientation::Xy,
            Self::Xz { .. } => OrthogonalOrientation::Xz,
            Self::Yz { .. } => OrthogonalOrientation::Yz,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Color(pub u8, pub u8, pub u8);

impl Color {
    pub const RED: Self = Self(220, 58, 52);
    pub const BROWN: Self = Self(117, 78, 47);
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Bead {
    pub position: GridPosition,
    pub color: Color,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ObjectRef {
    Group(ObjectId),
    Shape(ObjectId),
    Piece(ObjectId),
}

impl ObjectRef {
    #[must_use]
    pub const fn id(self) -> ObjectId {
        match self {
            Self::Group(id) | Self::Shape(id) | Self::Piece(id) => id,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum VoxelObjectRef {
    Shape(ObjectId),
    Piece(ObjectId),
}

impl VoxelObjectRef {
    #[must_use]
    pub const fn id(self) -> ObjectId {
        match self {
            Self::Shape(id) | Self::Piece(id) => id,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Assembly {
    pub id: ObjectId,
    pub name: String,
    pub root_group_id: ObjectId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Group {
    pub id: ObjectId,
    pub name: String,
    pub parent_group_id: Option<ObjectId>,
    #[serde(default)]
    pub sibling_order: u64,
    pub placement: Placement,
    pub visible: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Shape {
    pub id: ObjectId,
    pub name: String,
    pub parent_group_id: ObjectId,
    #[serde(default)]
    pub sibling_order: u64,
    pub placement: Placement,
    pub visible: bool,
    pub beads: BTreeMap<GridPosition, Color>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Piece {
    pub id: ObjectId,
    pub name: String,
    pub parent_group_id: ObjectId,
    #[serde(default)]
    pub sibling_order: u64,
    pub placement: Placement,
    pub visible: bool,
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
        Self::new_in_group(id, name, 0, plane, beads)
    }

    pub fn new_in_group(
        id: ObjectId,
        name: impl Into<String>,
        parent_group_id: ObjectId,
        plane: Plane,
        beads: impl IntoIterator<Item = Bead>,
    ) -> Result<Self, DomainError> {
        let piece = Self {
            id,
            name: name.into(),
            parent_group_id,
            sibling_order: 0,
            placement: Placement {
                orientation: plane.orientation(),
                ..Placement::default()
            },
            visible: true,
            plane,
            beads: beads
                .into_iter()
                .map(|bead| (bead.position, bead.color))
                .collect(),
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum PageSize {
    A4,
    Letter,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OutputSettings {
    pub page_size: PageSize,
    pub include_assembly_guide: bool,
}

impl Default for OutputSettings {
    fn default() -> Self {
        Self {
            page_size: PageSize::A4,
            include_assembly_guide: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OutputReadiness {
    pub assembly_id: ObjectId,
    pub remaining_shape_ids: Vec<ObjectId>,
}

impl OutputReadiness {
    #[must_use]
    pub const fn is_ready(&self) -> bool {
        self.remaining_shape_ids.is_empty()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Project {
    pub name: String,
    pub assembly: Assembly,
    pub groups: BTreeMap<ObjectId, Group>,
    pub shapes: BTreeMap<ObjectId, Shape>,
    pub pieces: BTreeMap<ObjectId, Piece>,
    pub output_settings: OutputSettings,
    next_id: ObjectId,
}

impl Project {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        let assembly_id = 1;
        let root_group_id = 2;
        Self {
            name: name.into(),
            assembly: Assembly {
                id: assembly_id,
                name: "Assembly".to_owned(),
                root_group_id,
            },
            groups: BTreeMap::from([(
                root_group_id,
                Group {
                    id: root_group_id,
                    name: "Root".to_owned(),
                    parent_group_id: None,
                    sibling_order: 0,
                    placement: Placement::default(),
                    visible: true,
                },
            )]),
            shapes: BTreeMap::new(),
            pieces: BTreeMap::new(),
            output_settings: OutputSettings::default(),
            next_id: 3,
        }
    }

    #[must_use]
    pub const fn root_group_id(&self) -> ObjectId {
        self.assembly.root_group_id
    }

    pub fn create_group(
        &mut self,
        parent_group_id: ObjectId,
        name: impl Into<String>,
    ) -> Result<ObjectId, DomainError> {
        self.require_group(parent_group_id)?;
        let sibling_order = self.next_sibling_order(parent_group_id);
        let id = self.allocate_id();
        self.groups.insert(
            id,
            Group {
                id,
                name: name.into(),
                parent_group_id: Some(parent_group_id),
                sibling_order,
                placement: Placement::default(),
                visible: true,
            },
        );
        Ok(id)
    }

    pub fn create_shape(&mut self, name: impl Into<String>) -> ObjectId {
        let root = self.root_group_id();
        self.create_shape_in_group(root, name)
            .expect("the root group always exists")
    }

    pub fn create_shape_in_group(
        &mut self,
        parent_group_id: ObjectId,
        name: impl Into<String>,
    ) -> Result<ObjectId, DomainError> {
        self.require_group(parent_group_id)?;
        let sibling_order = self.next_sibling_order(parent_group_id);
        let id = self.allocate_id();
        self.shapes.insert(
            id,
            Shape {
                id,
                name: name.into(),
                parent_group_id,
                sibling_order,
                placement: Placement::default(),
                visible: true,
                beads: BTreeMap::new(),
            },
        );
        Ok(id)
    }

    pub fn create_piece(
        &mut self,
        parent_group_id: ObjectId,
        name: impl Into<String>,
        plane: Plane,
    ) -> Result<ObjectId, DomainError> {
        self.require_group(parent_group_id)?;
        let sibling_order = self.next_sibling_order(parent_group_id);
        let id = self.allocate_id();
        let mut piece = Piece::new_in_group(id, name, parent_group_id, plane, [])?;
        piece.sibling_order = sibling_order;
        self.pieces.insert(id, piece);
        Ok(id)
    }

    pub fn delete_object(&mut self, object: ObjectRef) -> Result<(), DomainError> {
        match object {
            ObjectRef::Group(id) => {
                if id == self.root_group_id() {
                    return Err(DomainError::CannotDeleteRootGroup);
                }
                self.require_group(id)?;
                let has_children = self
                    .groups
                    .values()
                    .any(|group| group.parent_group_id == Some(id))
                    || self
                        .shapes
                        .values()
                        .any(|shape| shape.parent_group_id == id)
                    || self
                        .pieces
                        .values()
                        .any(|piece| piece.parent_group_id == id);
                if has_children {
                    return Err(DomainError::GroupIsNotEmpty(id));
                }
                self.groups.remove(&id);
            }
            ObjectRef::Shape(id) => {
                if self.shapes.remove(&id).is_none() {
                    return Err(DomainError::ShapeNotFound(id));
                }
            }
            ObjectRef::Piece(id) => {
                if self.pieces.remove(&id).is_none() {
                    return Err(DomainError::PieceNotFound(id));
                }
            }
        }
        Ok(())
    }

    pub fn rename_object(
        &mut self,
        object: ObjectRef,
        name: impl Into<String>,
    ) -> Result<(), DomainError> {
        let name = name.into();
        match object {
            ObjectRef::Group(id) => {
                self.groups
                    .get_mut(&id)
                    .ok_or(DomainError::GroupNotFound(id))?
                    .name = name;
            }
            ObjectRef::Shape(id) => {
                self.shapes
                    .get_mut(&id)
                    .ok_or(DomainError::ShapeNotFound(id))?
                    .name = name;
            }
            ObjectRef::Piece(id) => {
                self.pieces
                    .get_mut(&id)
                    .ok_or(DomainError::PieceNotFound(id))?
                    .name = name;
            }
        }
        Ok(())
    }

    pub fn add_shape_bead(&mut self, shape_id: ObjectId, bead: Bead) -> Result<(), DomainError> {
        self.add_bead(VoxelObjectRef::Shape(shape_id), bead)
    }

    pub fn add_bead(&mut self, target: VoxelObjectRef, bead: Bead) -> Result<(), DomainError> {
        match target {
            VoxelObjectRef::Shape(id) => {
                let shape = self
                    .shapes
                    .get_mut(&id)
                    .ok_or(DomainError::ShapeNotFound(id))?;
                if shape.beads.contains_key(&bead.position) {
                    return Err(DomainError::PositionOccupied {
                        object: target,
                        position: bead.position,
                    });
                }
                shape.beads.insert(bead.position, bead.color);
            }
            VoxelObjectRef::Piece(id) => {
                let piece = self
                    .pieces
                    .get_mut(&id)
                    .ok_or(DomainError::PieceNotFound(id))?;
                if !piece.plane.contains(bead.position) {
                    return Err(DomainError::PieceIsNotPlanar {
                        piece_id: id,
                        position: bead.position,
                    });
                }
                if piece.beads.contains_key(&bead.position) {
                    return Err(DomainError::PositionOccupied {
                        object: target,
                        position: bead.position,
                    });
                }
                piece.beads.insert(bead.position, bead.color);
            }
        }
        Ok(())
    }

    pub fn remove_bead(
        &mut self,
        target: VoxelObjectRef,
        position: GridPosition,
    ) -> Result<Color, DomainError> {
        let removed = match target {
            VoxelObjectRef::Shape(id) => self
                .shapes
                .get_mut(&id)
                .ok_or(DomainError::ShapeNotFound(id))?
                .beads
                .remove(&position),
            VoxelObjectRef::Piece(id) => self
                .pieces
                .get_mut(&id)
                .ok_or(DomainError::PieceNotFound(id))?
                .beads
                .remove(&position),
        };
        removed.ok_or(DomainError::BeadNotFound {
            object: target,
            position,
        })
    }

    pub fn recolor_bead(
        &mut self,
        target: VoxelObjectRef,
        position: GridPosition,
        color: Color,
    ) -> Result<(), DomainError> {
        let stored_color = match target {
            VoxelObjectRef::Shape(id) => self
                .shapes
                .get_mut(&id)
                .ok_or(DomainError::ShapeNotFound(id))?
                .beads
                .get_mut(&position),
            VoxelObjectRef::Piece(id) => self
                .pieces
                .get_mut(&id)
                .ok_or(DomainError::PieceNotFound(id))?
                .beads
                .get_mut(&position),
        }
        .ok_or(DomainError::BeadNotFound {
            object: target,
            position,
        })?;
        *stored_color = color;
        Ok(())
    }

    pub fn move_bead(
        &mut self,
        target: VoxelObjectRef,
        from: GridPosition,
        to: GridPosition,
    ) -> Result<(), DomainError> {
        if from == to {
            return self.bead_color(target, from).map(|_| ());
        }
        if let VoxelObjectRef::Piece(id) = target {
            let piece = self.pieces.get(&id).ok_or(DomainError::PieceNotFound(id))?;
            if !piece.plane.contains(to) {
                return Err(DomainError::PieceIsNotPlanar {
                    piece_id: id,
                    position: to,
                });
            }
        }
        if self.bead_color(target, to).is_ok() {
            return Err(DomainError::PositionOccupied {
                object: target,
                position: to,
            });
        }
        let color = self.remove_bead(target, from)?;
        self.add_bead(
            target,
            Bead {
                position: to,
                color,
            },
        )
    }

    pub fn set_placement(
        &mut self,
        object: ObjectRef,
        placement: Placement,
    ) -> Result<(), DomainError> {
        match object {
            ObjectRef::Group(id) => {
                self.groups
                    .get_mut(&id)
                    .ok_or(DomainError::GroupNotFound(id))?
                    .placement = placement;
            }
            ObjectRef::Shape(id) => {
                self.shapes
                    .get_mut(&id)
                    .ok_or(DomainError::ShapeNotFound(id))?
                    .placement = placement;
            }
            ObjectRef::Piece(id) => {
                self.pieces
                    .get_mut(&id)
                    .ok_or(DomainError::PieceNotFound(id))?
                    .placement = placement;
            }
        }
        Ok(())
    }

    pub fn rotate_object_quarter(
        &mut self,
        object: ObjectRef,
        axis: GridAxis,
        quarter_turns: i8,
    ) -> Result<(), DomainError> {
        let placement = match object {
            ObjectRef::Group(id) => {
                &mut self
                    .groups
                    .get_mut(&id)
                    .ok_or(DomainError::GroupNotFound(id))?
                    .placement
            }
            ObjectRef::Shape(id) => {
                &mut self
                    .shapes
                    .get_mut(&id)
                    .ok_or(DomainError::ShapeNotFound(id))?
                    .placement
            }
            ObjectRef::Piece(id) => {
                &mut self
                    .pieces
                    .get_mut(&id)
                    .ok_or(DomainError::PieceNotFound(id))?
                    .placement
            }
        };
        placement.rotation = placement.rotation.rotate_quarter(axis, quarter_turns);
        Ok(())
    }

    pub fn set_visibility(&mut self, object: ObjectRef, visible: bool) -> Result<(), DomainError> {
        match object {
            ObjectRef::Group(id) => {
                self.groups
                    .get_mut(&id)
                    .ok_or(DomainError::GroupNotFound(id))?
                    .visible = visible;
            }
            ObjectRef::Shape(id) => {
                self.shapes
                    .get_mut(&id)
                    .ok_or(DomainError::ShapeNotFound(id))?
                    .visible = visible;
            }
            ObjectRef::Piece(id) => {
                self.pieces
                    .get_mut(&id)
                    .ok_or(DomainError::PieceNotFound(id))?
                    .visible = visible;
            }
        }
        Ok(())
    }

    pub fn reparent_object(
        &mut self,
        object: ObjectRef,
        new_parent_group_id: ObjectId,
    ) -> Result<(), DomainError> {
        self.require_group(new_parent_group_id)?;
        let parent_changed = self.object_parent_group(object)? != Some(new_parent_group_id);
        let sibling_order = self.next_sibling_order(new_parent_group_id);
        match object {
            ObjectRef::Group(id) => {
                if id == self.root_group_id() {
                    return Err(DomainError::CannotReparentRootGroup);
                }
                self.require_group(id)?;
                let mut ancestor = Some(new_parent_group_id);
                while let Some(group_id) = ancestor {
                    if group_id == id {
                        return Err(DomainError::HierarchyCycle(id));
                    }
                    ancestor = self
                        .groups
                        .get(&group_id)
                        .and_then(|group| group.parent_group_id);
                }
                self.groups
                    .get_mut(&id)
                    .ok_or(DomainError::GroupNotFound(id))?
                    .parent_group_id = Some(new_parent_group_id);
                if parent_changed {
                    self.groups
                        .get_mut(&id)
                        .ok_or(DomainError::GroupNotFound(id))?
                        .sibling_order = sibling_order;
                }
            }
            ObjectRef::Shape(id) => {
                let shape = self
                    .shapes
                    .get_mut(&id)
                    .ok_or(DomainError::ShapeNotFound(id))?;
                shape.parent_group_id = new_parent_group_id;
                if parent_changed {
                    shape.sibling_order = sibling_order;
                }
            }
            ObjectRef::Piece(id) => {
                let piece = self
                    .pieces
                    .get_mut(&id)
                    .ok_or(DomainError::PieceNotFound(id))?;
                piece.parent_group_id = new_parent_group_id;
                if parent_changed {
                    piece.sibling_order = sibling_order;
                }
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn child_objects(&self, parent_group_id: ObjectId) -> Vec<ObjectRef> {
        let mut children = self
            .groups
            .values()
            .filter(|group| group.parent_group_id == Some(parent_group_id))
            .map(|group| (group.sibling_order, group.id, ObjectRef::Group(group.id)))
            .chain(
                self.shapes
                    .values()
                    .filter(|shape| shape.parent_group_id == parent_group_id)
                    .map(|shape| (shape.sibling_order, shape.id, ObjectRef::Shape(shape.id))),
            )
            .chain(
                self.pieces
                    .values()
                    .filter(|piece| piece.parent_group_id == parent_group_id)
                    .map(|piece| (piece.sibling_order, piece.id, ObjectRef::Piece(piece.id))),
            )
            .collect::<Vec<_>>();
        children.sort_by_key(|(order, id, _)| (*order, *id));
        children.into_iter().map(|(_, _, object)| object).collect()
    }

    pub fn move_object_after(
        &mut self,
        object: ObjectRef,
        target: ObjectRef,
    ) -> Result<(), DomainError> {
        if object == target {
            return Err(DomainError::InvalidSiblingTarget(target));
        }
        let parent_group_id = self
            .object_parent_group(target)?
            .ok_or(DomainError::InvalidSiblingTarget(target))?;
        self.reparent_object(object, parent_group_id)?;

        let mut siblings = self.child_objects(parent_group_id);
        siblings.retain(|candidate| *candidate != object);
        let target_index = siblings
            .iter()
            .position(|candidate| *candidate == target)
            .ok_or(DomainError::InvalidSiblingTarget(target))?;
        siblings.insert(target_index + 1, object);
        for (index, sibling) in siblings.into_iter().enumerate() {
            self.set_sibling_order(sibling, index as u64)?;
        }
        Ok(())
    }

    pub fn convert_shape_plane_to_piece(
        &mut self,
        shape_id: ObjectId,
        plane: Plane,
        name: impl Into<String>,
    ) -> Result<ObjectId, DomainError> {
        let (position_colors, parent_group_id, placement, visible) = {
            let shape = self
                .shapes
                .get(&shape_id)
                .ok_or(DomainError::ShapeNotFound(shape_id))?;
            (
                shape
                    .beads
                    .iter()
                    .filter(|(position, _)| plane.contains(**position))
                    .map(|(position, color)| (*position, *color))
                    .collect::<BTreeMap<_, _>>(),
                shape.parent_group_id,
                shape.placement,
                shape.visible,
            )
        };
        if position_colors.is_empty() {
            return Err(DomainError::NoBeadsOnPlane { shape_id, plane });
        }
        let piece_id = self.allocate_id();
        let mut piece = Piece::new_in_group(
            piece_id,
            name,
            parent_group_id,
            plane,
            position_colors.iter().map(|(position, color)| Bead {
                position: *position,
                color: *color,
            }),
        )?;
        piece.sibling_order = self.next_sibling_order(parent_group_id);
        piece.placement = placement;
        piece.placement.orientation = plane.orientation();
        piece.visible = visible;
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

    pub fn assembly_output_readiness(
        &self,
        assembly_id: ObjectId,
    ) -> Result<OutputReadiness, DomainError> {
        if assembly_id != self.assembly.id {
            return Err(DomainError::AssemblyNotFound(assembly_id));
        }
        Ok(OutputReadiness {
            assembly_id,
            remaining_shape_ids: self
                .shapes
                .values()
                .filter(|shape| !shape.beads.is_empty())
                .map(|shape| shape.id)
                .collect(),
        })
    }

    #[must_use]
    pub fn output_readiness(&self) -> OutputReadiness {
        self.assembly_output_readiness(self.assembly.id)
            .expect("the project's assembly always exists")
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

    pub fn validate(&self) -> Result<(), DomainError> {
        if self.assembly.root_group_id == 0
            || !self.groups.contains_key(&self.assembly.root_group_id)
        {
            return Err(DomainError::InvalidRootGroup(self.assembly.root_group_id));
        }
        let root = &self.groups[&self.assembly.root_group_id];
        if root.parent_group_id.is_some() {
            return Err(DomainError::InvalidRootGroup(root.id));
        }

        let mut ids = BTreeSet::from([self.assembly.id]);
        for (id, group) in &self.groups {
            self.validate_map_id(*id, group.id)?;
            if !ids.insert(*id) {
                return Err(DomainError::DuplicateObjectId(*id));
            }
            if !group.placement.rotation.is_valid() {
                return Err(DomainError::InvalidGridRotation(ObjectRef::Group(*id)));
            }
            if *id != self.root_group_id() {
                let parent = group
                    .parent_group_id
                    .ok_or(DomainError::GroupNotFound(*id))?;
                self.require_group(parent)?;
            }
            let mut seen = BTreeSet::new();
            let mut current = group.parent_group_id;
            while let Some(group_id) = current {
                if !seen.insert(group_id) {
                    return Err(DomainError::HierarchyCycle(*id));
                }
                current = self
                    .groups
                    .get(&group_id)
                    .ok_or(DomainError::GroupNotFound(group_id))?
                    .parent_group_id;
            }
        }
        for (id, shape) in &self.shapes {
            self.validate_map_id(*id, shape.id)?;
            if !ids.insert(*id) {
                return Err(DomainError::DuplicateObjectId(*id));
            }
            if !shape.placement.rotation.is_valid() {
                return Err(DomainError::InvalidGridRotation(ObjectRef::Shape(*id)));
            }
            self.require_group(shape.parent_group_id)?;
        }
        for (id, piece) in &self.pieces {
            self.validate_map_id(*id, piece.id)?;
            if !ids.insert(*id) {
                return Err(DomainError::DuplicateObjectId(*id));
            }
            if !piece.placement.rotation.is_valid() {
                return Err(DomainError::InvalidGridRotation(ObjectRef::Piece(*id)));
            }
            self.require_group(piece.parent_group_id)?;
            piece.validate()?;
        }
        if ids
            .iter()
            .next_back()
            .is_some_and(|max_id| self.next_id <= *max_id)
        {
            return Err(DomainError::InvalidNextId(self.next_id));
        }
        Ok(())
    }

    pub fn from_legacy(
        name: String,
        shapes: BTreeMap<ObjectId, (String, BTreeMap<GridPosition, Color>)>,
        pieces: BTreeMap<ObjectId, (String, Plane, BTreeMap<GridPosition, Color>)>,
        legacy_next_id: ObjectId,
    ) -> Result<Self, DomainError> {
        let max_legacy_id = shapes
            .keys()
            .chain(pieces.keys())
            .copied()
            .max()
            .unwrap_or(0);
        let assembly_id = legacy_next_id.max(max_legacy_id.saturating_add(1));
        let root_group_id = assembly_id.saturating_add(1);
        let next_id = root_group_id.saturating_add(1);
        let project = Self {
            name,
            assembly: Assembly {
                id: assembly_id,
                name: "Assembly".to_owned(),
                root_group_id,
            },
            groups: BTreeMap::from([(
                root_group_id,
                Group {
                    id: root_group_id,
                    name: "Root".to_owned(),
                    parent_group_id: None,
                    sibling_order: 0,
                    placement: Placement::default(),
                    visible: true,
                },
            )]),
            shapes: shapes
                .into_iter()
                .map(|(id, (name, beads))| {
                    (
                        id,
                        Shape {
                            id,
                            name,
                            parent_group_id: root_group_id,
                            sibling_order: id,
                            placement: Placement::default(),
                            visible: true,
                            beads,
                        },
                    )
                })
                .collect(),
            pieces: pieces
                .into_iter()
                .map(|(id, (name, plane, beads))| {
                    (
                        id,
                        Piece {
                            id,
                            name,
                            parent_group_id: root_group_id,
                            sibling_order: id,
                            placement: Placement {
                                orientation: plane.orientation(),
                                ..Placement::default()
                            },
                            visible: true,
                            plane,
                            beads,
                        },
                    )
                })
                .collect(),
            output_settings: OutputSettings::default(),
            next_id,
        };
        project.validate()?;
        Ok(project)
    }

    fn bead_color(
        &self,
        target: VoxelObjectRef,
        position: GridPosition,
    ) -> Result<Color, DomainError> {
        let color = match target {
            VoxelObjectRef::Shape(id) => self
                .shapes
                .get(&id)
                .ok_or(DomainError::ShapeNotFound(id))?
                .beads
                .get(&position),
            VoxelObjectRef::Piece(id) => self
                .pieces
                .get(&id)
                .ok_or(DomainError::PieceNotFound(id))?
                .beads
                .get(&position),
        };
        color.copied().ok_or(DomainError::BeadNotFound {
            object: target,
            position,
        })
    }

    fn require_group(&self, id: ObjectId) -> Result<(), DomainError> {
        if self.groups.contains_key(&id) {
            Ok(())
        } else {
            Err(DomainError::GroupNotFound(id))
        }
    }

    fn object_parent_group(&self, object: ObjectRef) -> Result<Option<ObjectId>, DomainError> {
        match object {
            ObjectRef::Group(id) => self
                .groups
                .get(&id)
                .map(|group| group.parent_group_id)
                .ok_or(DomainError::GroupNotFound(id)),
            ObjectRef::Shape(id) => self
                .shapes
                .get(&id)
                .map(|shape| Some(shape.parent_group_id))
                .ok_or(DomainError::ShapeNotFound(id)),
            ObjectRef::Piece(id) => self
                .pieces
                .get(&id)
                .map(|piece| Some(piece.parent_group_id))
                .ok_or(DomainError::PieceNotFound(id)),
        }
    }

    fn next_sibling_order(&self, parent_group_id: ObjectId) -> u64 {
        self.child_objects(parent_group_id)
            .into_iter()
            .filter_map(|object| match object {
                ObjectRef::Group(id) => self.groups.get(&id).map(|group| group.sibling_order),
                ObjectRef::Shape(id) => self.shapes.get(&id).map(|shape| shape.sibling_order),
                ObjectRef::Piece(id) => self.pieces.get(&id).map(|piece| piece.sibling_order),
            })
            .max()
            .map_or(0, |order| order.saturating_add(1))
    }

    fn set_sibling_order(
        &mut self,
        object: ObjectRef,
        sibling_order: u64,
    ) -> Result<(), DomainError> {
        match object {
            ObjectRef::Group(id) => {
                self.groups
                    .get_mut(&id)
                    .ok_or(DomainError::GroupNotFound(id))?
                    .sibling_order = sibling_order;
            }
            ObjectRef::Shape(id) => {
                self.shapes
                    .get_mut(&id)
                    .ok_or(DomainError::ShapeNotFound(id))?
                    .sibling_order = sibling_order;
            }
            ObjectRef::Piece(id) => {
                self.pieces
                    .get_mut(&id)
                    .ok_or(DomainError::PieceNotFound(id))?
                    .sibling_order = sibling_order;
            }
        }
        Ok(())
    }

    fn validate_map_id(&self, key: ObjectId, actual: ObjectId) -> Result<(), DomainError> {
        if key == actual {
            Ok(())
        } else {
            Err(DomainError::ObjectIdMismatch { key, actual })
        }
    }

    fn allocate_id(&mut self) -> ObjectId {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DomainError {
    AssemblyNotFound(ObjectId),
    GroupNotFound(ObjectId),
    ShapeNotFound(ObjectId),
    PieceNotFound(ObjectId),
    BeadNotFound {
        object: VoxelObjectRef,
        position: GridPosition,
    },
    PositionOccupied {
        object: VoxelObjectRef,
        position: GridPosition,
    },
    PieceIsNotPlanar {
        piece_id: ObjectId,
        position: GridPosition,
    },
    NoBeadsOnPlane {
        shape_id: ObjectId,
        plane: Plane,
    },
    CannotDeleteRootGroup,
    CannotReparentRootGroup,
    InvalidSiblingTarget(ObjectRef),
    InvalidGridRotation(ObjectRef),
    GroupIsNotEmpty(ObjectId),
    HierarchyCycle(ObjectId),
    InvalidRootGroup(ObjectId),
    DuplicateObjectId(ObjectId),
    ObjectIdMismatch {
        key: ObjectId,
        actual: ObjectId,
    },
    InvalidNextId(ObjectId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BeadInventory {
    pub by_color: BTreeMap<Color, usize>,
    pub total: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StructuralWarning {
    ShallowJoint {
        piece_a: ObjectId,
        piece_b: ObjectId,
    },
    UnsupportedPiece {
        piece_id: ObjectId,
    },
}

impl Project {
    #[must_use]
    pub fn inventory(&self) -> BeadInventory {
        let mut by_color = BTreeMap::new();
        for piece in self.pieces.values() {
            for color in piece.beads.values() {
                *by_color.entry(*color).or_insert(0) += 1;
            }
        }
        BeadInventory {
            total: by_color.values().sum(),
            by_color,
        }
    }

    #[must_use]
    pub fn structural_warnings(&self) -> Vec<StructuralWarning> {
        let pieces = self.pieces.values().collect::<Vec<_>>();
        let mut warnings = Vec::new();
        for (index, a) in pieces.iter().enumerate() {
            for b in pieces.iter().skip(index + 1) {
                let contacts = a
                    .beads
                    .keys()
                    .filter(|pa| {
                        b.beads.keys().any(|pb| {
                            (pa.x - pb.x).abs() + (pa.y - pb.y).abs() + (pa.z - pb.z).abs() == 1
                        })
                    })
                    .count();
                if contacts == 1 {
                    warnings.push(StructuralWarning::ShallowJoint {
                        piece_a: a.id,
                        piece_b: b.id,
                    });
                }
            }
        }
        for piece in &pieces {
            if !piece.beads.is_empty()
                && !pieces.iter().any(|other| {
                    other.id != piece.id
                        && piece.beads.keys().any(|p| {
                            other
                                .beads
                                .keys()
                                .any(|q| p.z > q.z && (p.x - q.x).abs() + (p.y - q.y).abs() <= 1)
                        })
                })
            {
                warnings.push(StructuralWarning::UnsupportedPiece { piece_id: piece.id });
            }
        }
        warnings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_an_empty_valid_project_with_root_hierarchy() {
        let project = Project::new("empty");
        assert!(project.shapes.is_empty());
        assert!(project.pieces.is_empty());
        assert!(project.validate().is_ok());
        assert_eq!(
            project.groups[&project.root_group_id()].parent_group_id,
            None
        );
    }

    #[test]
    fn rejects_non_planar_piece_on_creation_and_update() {
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

        let mut project = Project::new("piece");
        let piece = project
            .create_piece(project.root_group_id(), "flat", Plane::Xy { z: 0 })
            .unwrap();
        assert_eq!(
            project.add_bead(
                VoxelObjectRef::Piece(piece),
                Bead {
                    position: GridPosition::new(0, 0, 1),
                    color: Color::RED,
                },
            ),
            Err(DomainError::PieceIsNotPlanar {
                piece_id: piece,
                position: GridPosition::new(0, 0, 1),
            })
        );
        assert!(project.pieces[&piece].beads.is_empty());
    }

    #[test]
    fn assembly_output_reports_shapes_and_becomes_ready_after_conversion() {
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
        let readiness = project
            .assembly_output_readiness(project.assembly.id)
            .unwrap();
        assert_eq!(readiness.remaining_shape_ids, vec![shape_id]);
        assert!(!readiness.is_ready());

        project
            .convert_shape_plane_to_piece(shape_id, Plane::Xy { z: 0 }, "head")
            .unwrap();
        assert!(project.output_readiness().is_ready());
    }

    #[test]
    fn value_objects_and_serialization_are_deterministic() {
        let build = |reverse: bool| {
            let mut project = Project::new("deterministic");
            let shape = project.create_shape("shape");
            let beads = [
                Bead {
                    position: GridPosition::new(-1, 2, 0),
                    color: Color::RED,
                },
                Bead {
                    position: GridPosition::new(3, 1, 0),
                    color: Color::BROWN,
                },
            ];
            let order = if reverse { [beads[1], beads[0]] } else { beads };
            for bead in order {
                project.add_shape_bead(shape, bead).unwrap();
            }
            project
        };
        let first = build(false);
        let second = build(true);
        assert_eq!(first, second);
        assert_eq!(
            serde_json::to_string(&first).unwrap(),
            serde_json::to_string(&second).unwrap()
        );
    }

    #[test]
    fn hierarchy_rejects_cycles() {
        let mut project = Project::new("groups");
        let parent = project
            .create_group(project.root_group_id(), "parent")
            .unwrap();
        let child = project.create_group(parent, "child").unwrap();
        assert_eq!(
            project.reparent_object(ObjectRef::Group(parent), child),
            Err(DomainError::HierarchyCycle(parent))
        );
        assert!(project.validate().is_ok());
    }

    #[test]
    fn sibling_order_can_move_objects_after_mixed_object_types() {
        let mut project = Project::new("ordered");
        let root = project.root_group_id();
        let piece = project
            .create_piece(root, "piece", Plane::Xy { z: 0 })
            .unwrap();
        let group = project.create_group(root, "group").unwrap();
        let shape = project.create_shape_in_group(root, "shape").unwrap();
        assert_eq!(
            project.child_objects(root),
            vec![
                ObjectRef::Piece(piece),
                ObjectRef::Group(group),
                ObjectRef::Shape(shape),
            ]
        );

        project
            .move_object_after(ObjectRef::Piece(piece), ObjectRef::Shape(shape))
            .unwrap();
        assert_eq!(
            project.child_objects(root),
            vec![
                ObjectRef::Group(group),
                ObjectRef::Shape(shape),
                ObjectRef::Piece(piece),
            ]
        );
    }

    #[test]
    fn moving_after_an_object_in_another_group_reparents_it() {
        let mut project = Project::new("ordered");
        let root = project.root_group_id();
        let left = project.create_group(root, "left").unwrap();
        let right = project.create_group(root, "right").unwrap();
        let moving = project
            .create_piece(left, "moving", Plane::Xy { z: 0 })
            .unwrap();
        let target = project
            .create_piece(right, "target", Plane::Xy { z: 0 })
            .unwrap();

        project
            .move_object_after(ObjectRef::Piece(moving), ObjectRef::Piece(target))
            .unwrap();
        assert!(project.child_objects(left).is_empty());
        assert_eq!(
            project.child_objects(right),
            vec![ObjectRef::Piece(target), ObjectRef::Piece(moving)]
        );
        assert_eq!(project.pieces[&moving].parent_group_id, right);
    }

    #[test]
    fn quarter_turns_form_the_24_valid_cube_rotations() {
        let rotated = GridRotation::IDENTITY.rotate_quarter(GridAxis::Z, 1);
        assert_eq!(
            rotated.apply(GridPosition::new(1, 2, 3)),
            GridPosition::new(-2, 1, 3)
        );
        assert_eq!(
            GridRotation::IDENTITY.rotate_quarter(GridAxis::X, 4),
            GridRotation::IDENTITY
        );

        let mut rotations = BTreeSet::new();
        for x in 0..4 {
            for y in 0..4 {
                for z in 0..4 {
                    let rotation = GridRotation::IDENTITY
                        .rotate_quarter(GridAxis::X, x)
                        .rotate_quarter(GridAxis::Y, y)
                        .rotate_quarter(GridAxis::Z, z);
                    assert!(rotation.is_valid());
                    rotations.insert(rotation);
                }
            }
        }
        assert_eq!(rotations.len(), 24);
    }

    #[test]
    fn inverse_rotation_restores_grid_positions() {
        let rotation = GridRotation::IDENTITY
            .rotate_quarter(GridAxis::X, 1)
            .rotate_quarter(GridAxis::Z, 3);
        let position = GridPosition::new(3, -5, 7);

        assert_eq!(rotation.inverse().apply(rotation.apply(position)), position);
    }

    #[test]
    fn projects_without_rotation_data_load_as_identity() {
        let project = Project::new("legacy v2");
        let root = project.root_group_id().to_string();
        let mut value = serde_json::to_value(project).unwrap();
        value["groups"][&root]["placement"]
            .as_object_mut()
            .unwrap()
            .remove("rotation");

        let loaded: Project = serde_json::from_value(value).unwrap();
        assert_eq!(
            loaded.groups[&loaded.root_group_id()].placement.rotation,
            GridRotation::IDENTITY
        );
        assert!(loaded.validate().is_ok());
    }
}
