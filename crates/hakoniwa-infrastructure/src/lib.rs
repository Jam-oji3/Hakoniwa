//! `.ibcad` persistence for the MVP.

use hakoniwa_domain::Project;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

const FORMAT_VERSION: u32 = 1;

#[derive(Debug)]
pub enum IbcadError {
    Io(std::io::Error),
    Zip(zip::result::ZipError),
    Json(serde_json::Error),
    UnsupportedVersion(u32),
    InvalidProject,
}
impl From<std::io::Error> for IbcadError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}
impl From<zip::result::ZipError> for IbcadError {
    fn from(value: zip::result::ZipError) -> Self {
        Self::Zip(value)
    }
}
impl From<serde_json::Error> for IbcadError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

#[derive(Deserialize, Serialize)]
struct StoredProject {
    format_version: u32,
    project: Project,
}

pub fn save(path: impl AsRef<Path>, project: &Project) -> Result<(), IbcadError> {
    let file = File::create(path)?;
    let mut zip = ZipWriter::new(file);
    zip.start_file("project.json", SimpleFileOptions::default())?;
    let json = serde_json::to_vec_pretty(&StoredProject {
        format_version: FORMAT_VERSION,
        project: project.clone(),
    })?;
    zip.write_all(&json)?;
    zip.finish()?;
    Ok(())
}

pub fn load(path: impl AsRef<Path>) -> Result<Project, IbcadError> {
    let file = File::open(path)?;
    let mut zip = ZipArchive::new(file)?;
    let mut json = Vec::new();
    zip.by_name("project.json")?.read_to_end(&mut json)?;
    let stored: StoredProject = serde_json::from_slice(&json)?;
    if stored.format_version != FORMAT_VERSION {
        return Err(IbcadError::UnsupportedVersion(stored.format_version));
    }
    if stored
        .project
        .pieces
        .values()
        .any(|piece| piece.validate().is_err())
    {
        return Err(IbcadError::InvalidProject);
    }
    Ok(stored.project)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hakoniwa_domain::{Bead, Color, GridPosition};

    #[test]
    fn round_trip_preserves_project() {
        let path = std::env::temp_dir().join("hakoniwa-persistence-test.ibcad");
        let mut project = Project::new("hammer");
        let shape = project.create_shape("head");
        project
            .add_shape_bead(
                shape,
                Bead {
                    position: GridPosition::new(0, 0, 0),
                    color: Color::RED,
                },
            )
            .unwrap();
        save(&path, &project).unwrap();
        assert_eq!(load(&path).unwrap(), project);
        std::fs::remove_file(path).unwrap();
    }
}
