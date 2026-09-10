//! Versioned and deterministic .ibcad project persistence.

use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Seek, Write},
    path::Path,
};

use hakoniwa_application::ProjectRepository;
use hakoniwa_domain::{Color, DomainError, GridPosition, ObjectId, Plane, Project};
use serde::{Deserialize, Serialize};
use zip::{
    CompressionMethod, DateTime, ZipArchive, ZipWriter, result::ZipError, write::SimpleFileOptions,
};

pub const CURRENT_FORMAT_VERSION: u32 = 2;
const PROJECT_ENTRY: &str = "project.json";

#[derive(Debug)]
pub enum IbcadError {
    Io(std::io::Error),
    Zip(ZipError),
    Json(serde_json::Error),
    MissingProjectJson,
    UnsupportedVersion(u32),
    InvalidProject(DomainError),
}

impl From<std::io::Error> for IbcadError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<ZipError> for IbcadError {
    fn from(value: ZipError) -> Self {
        Self::Zip(value)
    }
}

impl From<serde_json::Error> for IbcadError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<DomainError> for IbcadError {
    fn from(value: DomainError) -> Self {
        Self::InvalidProject(value)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct IbcadRepository;

impl ProjectRepository for IbcadRepository {
    type Error = IbcadError;

    fn save(&self, path: &Path, project: &Project) -> Result<(), Self::Error> {
        save_project(path, project)
    }

    fn load(&self, path: &Path) -> Result<Project, Self::Error> {
        load_project(path)
    }
}

#[derive(Deserialize)]
struct VersionHeader {
    format_version: u32,
}

#[derive(Deserialize, Serialize)]
struct StoredProjectV2 {
    format_version: u32,
    project: Project,
}

#[derive(Deserialize, Serialize)]
struct StoredProjectV1 {
    format_version: u32,
    project: LegacyProject,
}

#[derive(Deserialize, Serialize)]
struct LegacyProject {
    name: String,
    shapes: BTreeMap<ObjectId, LegacyShape>,
    pieces: BTreeMap<ObjectId, LegacyPiece>,
    next_id: ObjectId,
}

#[derive(Deserialize, Serialize)]
struct LegacyShape {
    id: ObjectId,
    name: String,
    beads: BTreeMap<GridPosition, Color>,
}

#[derive(Deserialize, Serialize)]
struct LegacyPiece {
    id: ObjectId,
    name: String,
    plane: Plane,
    beads: BTreeMap<GridPosition, Color>,
}

pub fn save(path: impl AsRef<Path>, project: &Project) -> Result<(), IbcadError> {
    save_project(path.as_ref(), project)
}

pub fn load(path: impl AsRef<Path>) -> Result<Project, IbcadError> {
    load_project(path.as_ref())
}

fn save_project(path: &Path, project: &Project) -> Result<(), IbcadError> {
    project.validate()?;
    let file = File::create(path)?;
    write_current_archive(file, project)
}

fn write_current_archive<W: Write + Seek>(writer: W, project: &Project) -> Result<(), IbcadError> {
    let mut zip = ZipWriter::new(writer);
    zip.start_file(PROJECT_ENTRY, deterministic_options())?;
    let json = serde_json::to_vec_pretty(&StoredProjectV2 {
        format_version: CURRENT_FORMAT_VERSION,
        project: project.clone(),
    })?;
    zip.write_all(&json)?;
    zip.finish()?;
    Ok(())
}

fn deterministic_options() -> SimpleFileOptions {
    SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(DateTime::default())
        .unix_permissions(0o644)
}

fn load_project(path: &Path) -> Result<Project, IbcadError> {
    let file = File::open(path)?;
    let mut zip = ZipArchive::new(file)?;
    let mut json = Vec::new();
    match zip.by_name(PROJECT_ENTRY) {
        Ok(mut entry) => {
            entry.read_to_end(&mut json)?;
        }
        Err(ZipError::FileNotFound) => return Err(IbcadError::MissingProjectJson),
        Err(error) => return Err(IbcadError::Zip(error)),
    }

    let header: VersionHeader = serde_json::from_slice(&json)?;
    let project = match header.format_version {
        1 => migrate_v1(serde_json::from_slice::<StoredProjectV1>(&json)?.project)?,
        CURRENT_FORMAT_VERSION => serde_json::from_slice::<StoredProjectV2>(&json)?.project,
        version => return Err(IbcadError::UnsupportedVersion(version)),
    };
    project.validate()?;
    Ok(project)
}

fn migrate_v1(legacy: LegacyProject) -> Result<Project, IbcadError> {
    let mut shapes = BTreeMap::new();
    for (key, shape) in legacy.shapes {
        if key != shape.id {
            return Err(DomainError::ObjectIdMismatch {
                key,
                actual: shape.id,
            }
            .into());
        }
        shapes.insert(key, (shape.name, shape.beads));
    }

    let mut pieces = BTreeMap::new();
    for (key, piece) in legacy.pieces {
        if key != piece.id {
            return Err(DomainError::ObjectIdMismatch {
                key,
                actual: piece.id,
            }
            .into());
        }
        pieces.insert(key, (piece.name, piece.plane, piece.beads));
    }

    Project::from_legacy(legacy.name, shapes, pieces, legacy.next_id).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use hakoniwa_application::{Command, Editor};
    use hakoniwa_domain::{
        Bead, Color, GridPosition, ObjectRef, OrthogonalOrientation, OutputSettings, PageSize,
        Placement, VoxelObjectRef,
    };

    use super::*;

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

    fn temp_path(label: &str) -> std::path::PathBuf {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "hakoniwa-{label}-{}-{id}.ibcad",
            std::process::id()
        ))
    }

    fn complex_project() -> Project {
        let mut project = Project::new("hammer");
        let group = project
            .create_group(project.root_group_id(), "head assembly")
            .unwrap();
        let head = project
            .create_piece(group, "head", Plane::Xy { z: 0 })
            .unwrap();
        let handle = project
            .create_piece(project.root_group_id(), "handle", Plane::Xz { y: 0 })
            .unwrap();
        let draft = project.create_shape_in_group(group, "draft cap").unwrap();

        project
            .add_bead(
                VoxelObjectRef::Piece(head),
                Bead {
                    position: GridPosition::ZERO,
                    color: Color::RED,
                },
            )
            .unwrap();
        project
            .add_bead(
                VoxelObjectRef::Piece(handle),
                Bead {
                    position: GridPosition::new(0, 0, 1),
                    color: Color::BROWN,
                },
            )
            .unwrap();
        project
            .add_bead(
                VoxelObjectRef::Shape(draft),
                Bead {
                    position: GridPosition::new(2, 2, 2),
                    color: Color::RED,
                },
            )
            .unwrap();
        project
            .set_placement(
                ObjectRef::Piece(handle),
                Placement {
                    translation: GridPosition::new(5, -2, 1),
                    orientation: OrthogonalOrientation::Yz,
                },
            )
            .unwrap();
        project
            .set_visibility(ObjectRef::Shape(draft), false)
            .unwrap();
        project.output_settings = OutputSettings {
            page_size: PageSize::Letter,
            include_assembly_guide: false,
        };
        project
    }

    fn write_json_archive(path: &Path, json: &[u8]) {
        let file = File::create(path).unwrap();
        let mut zip = ZipWriter::new(file);
        zip.start_file(PROJECT_ENTRY, deterministic_options())
            .unwrap();
        zip.write_all(json).unwrap();
        zip.finish().unwrap();
    }

    #[test]
    fn empty_and_complex_projects_round_trip_exactly() {
        for (label, project) in [
            ("empty", Project::new("empty")),
            ("complex", complex_project()),
        ] {
            let path = temp_path(label);
            save(&path, &project).unwrap();
            assert_eq!(load(&path).unwrap(), project);
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn corrupt_missing_and_unknown_versions_are_rejected() {
        let corrupt = temp_path("corrupt");
        fs::write(&corrupt, b"not a zip").unwrap();
        assert!(matches!(load(&corrupt), Err(IbcadError::Zip(_))));
        fs::remove_file(corrupt).unwrap();

        let missing = temp_path("missing");
        ZipWriter::new(File::create(&missing).unwrap())
            .finish()
            .unwrap();
        assert!(matches!(
            load(&missing),
            Err(IbcadError::MissingProjectJson)
        ));
        fs::remove_file(missing).unwrap();

        let unknown = temp_path("unknown");
        write_json_archive(&unknown, br#"{"format_version":999,"project":{}}"#);
        assert!(matches!(
            load(&unknown),
            Err(IbcadError::UnsupportedVersion(999))
        ));
        fs::remove_file(unknown).unwrap();
    }

    #[test]
    fn invalid_piece_is_rejected_before_save() {
        let path = temp_path("invalid");
        let mut project = complex_project();
        let piece = project.pieces.values_mut().next().unwrap();
        piece.beads.insert(GridPosition::new(0, 0, 99), Color::RED);
        assert!(matches!(
            save(&path, &project),
            Err(IbcadError::InvalidProject(
                DomainError::PieceIsNotPlanar { .. }
            ))
        ));
        assert!(!path.exists());
    }

    #[test]
    fn saving_the_same_project_is_byte_deterministic() {
        let first = temp_path("deterministic-a");
        let second = temp_path("deterministic-b");
        let project = complex_project();
        save(&first, &project).unwrap();
        save(&second, &project).unwrap();
        assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
        fs::remove_file(first).unwrap();
        fs::remove_file(second).unwrap();
    }

    #[test]
    fn version_one_projects_are_migrated_to_the_root_group() {
        let path = temp_path("v1");
        let legacy = StoredProjectV1 {
            format_version: 1,
            project: LegacyProject {
                name: "legacy".to_owned(),
                shapes: BTreeMap::from([(
                    1,
                    LegacyShape {
                        id: 1,
                        name: "draft".to_owned(),
                        beads: BTreeMap::from([(GridPosition::ZERO, Color::RED)]),
                    },
                )]),
                pieces: BTreeMap::new(),
                next_id: 2,
            },
        };
        write_json_archive(&path, &serde_json::to_vec(&legacy).unwrap());
        let project = load(&path).unwrap();
        assert_eq!(project.shapes[&1].parent_group_id, project.root_group_id());
        assert!(project.validate().is_ok());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn a_loaded_project_can_continue_through_application_commands() {
        let path = temp_path("continue");
        let project = complex_project();
        let shape = *project.shapes.keys().next().unwrap();
        save(&path, &project).unwrap();
        let mut editor = Editor::new(load(&path).unwrap());
        editor
            .execute(Command::AddBead {
                target: VoxelObjectRef::Shape(shape),
                bead: Bead {
                    position: GridPosition::new(3, 3, 3),
                    color: Color::BROWN,
                },
            })
            .unwrap();
        assert!(editor.can_undo());
        assert_eq!(editor.project().bead_count(), project.bead_count() + 1);
        fs::remove_file(path).unwrap();
    }
}
