//! Versioned .avproj ZIP: normalized original RGBA PNG, parameters and bound edits.
use crate::{
    decode,
    editor::{RecordedEdit, Revision},
};
use av_core::{Params, RgbaImage};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

/// Project errors preserve their decoder, ZIP or filesystem source.
pub type ProjectResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
const IMAGE_LIMIT: u64 = 300_000_000;
const EDIT_LIMIT: u64 = 8_000_000;
const ARCHIVE_LIMIT: u64 = IMAGE_LIMIT + EDIT_LIMIT + 100_000;
const MEMBERS: [&str; 4] = ["manifest.json", "original.png", "params.json", "edits.json"];
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format: String,
    version: u32,
    width: u32,
    height: u32,
    name: String,
}
/// Validated project inputs. Reconstruction uses Session::restore on a worker.
pub struct Project {
    pub image: RgbaImage,
    pub params: Params,
    pub edits: Vec<RecordedEdit>,
    pub name: String,
}
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 4096
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\', '\0'])
}
fn invalid(message: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message)
}

/// Write atomically beside the destination; failed saves preserve the previous file.
pub fn save(path: &Path, name: &str, image: &RgbaImage, revision: &Revision) -> ProjectResult<()> {
    image.validate(av_core::raster::DEFAULT_PIXEL_LIMIT)?;
    revision.params.validate()?;
    if !valid_name(name) || revision.edits.len() > 10_000 {
        return Err(invalid("project metadata limit").into());
    }
    let manifest = Manifest {
        format: "antivector".into(),
        version: 1,
        width: image.w,
        height: image.h,
        name: name.into(),
    };
    let image = decode::png_bytes(image)?;
    let params = serde_json::to_vec(&revision.params)?;
    let edits = serde_json::to_vec(&revision.edits)?;
    if image.len() as u64 > IMAGE_LIMIT || params.len() > 64_000 || edits.len() as u64 > EDIT_LIMIT
    {
        return Err(invalid("project entry too large").into());
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    {
        let mut zip = zip::ZipWriter::new(temporary.as_file_mut());
        // PNG is already compressed; Stored also keeps the reader's memory bounded.
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, bytes) in
            MEMBERS
                .into_iter()
                .zip([serde_json::to_vec(&manifest)?, image, params, edits])
        {
            zip.start_file(name, options)?;
            zip.write_all(&bytes)?;
        }
        zip.finish()?;
    }
    temporary.as_file_mut().sync_all()?;
    temporary.persist(path)?;
    Ok(())
}

// Bound central-directory allocation before asking the ZIP parser to create entries.
// v1 writes four non-ZIP64 entries and never needs split archives or archive comments.
fn check_directory(file: &mut (impl Read + Seek), size: u64) -> ProjectResult<()> {
    if !(22..=ARCHIVE_LIMIT).contains(&size) {
        return Err(invalid("project archive size limit").into());
    }
    file.seek(SeekFrom::End(-22))?;
    let mut end = [0_u8; 22];
    file.read_exact(&mut end)?;
    let u16_at = |i| u16::from_le_bytes([end[i], end[i + 1]]);
    let u32_at = |i| u32::from_le_bytes([end[i], end[i + 1], end[i + 2], end[i + 3]]);
    if &end[..4] != b"PK\x05\x06"
        || u16_at(4) != 0
        || u16_at(6) != 0
        || u16_at(8) != 4
        || u16_at(10) != 4
        || u16_at(20) != 0
        || u32_at(12) > 16_384
        || u64::from(u32_at(16)) + u64::from(u32_at(12)) != size - 22
    {
        return Err(invalid("unsupported or oversized project directory").into());
    }
    file.seek(SeekFrom::Start(0))?;
    Ok(())
}
/// Read only fixed members in memory; no ZIP paths are extracted to the filesystem.
pub fn open(path: &Path) -> ProjectResult<Project> {
    let file = File::open(path)?;
    let size = file.metadata()?.len();
    open_reader(file, size)
}
/// Open a dropped project buffer with the same limits as file-based loading.
pub fn open_bytes(bytes: &[u8]) -> ProjectResult<Project> {
    open_reader(std::io::Cursor::new(bytes), bytes.len() as u64)
}
fn open_reader(mut file: impl Read + Seek, size: u64) -> ProjectResult<Project> {
    check_directory(&mut file, size)?;
    let mut zip = zip::ZipArchive::new(file)?;
    if zip.len() != 4 {
        return Err(invalid("project must contain four members").into());
    }
    let mut seen = BTreeSet::new();
    for i in 0..zip.len() {
        let entry = zip.by_index(i)?;
        let name = entry.name()?;
        if !MEMBERS.contains(&name.as_ref())
            || !seen.insert(name.into_owned())
            || entry.is_dir()
            || entry.encrypted()
            || entry.compression() != zip::CompressionMethod::Stored
            || entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
        {
            return Err(invalid("unexpected project member").into());
        }
    }
    let mut read = |name: &str, limit: u64| -> ProjectResult<Vec<u8>> {
        let mut entry = zip.by_name(name)?;
        if entry.size() > limit {
            return Err(invalid("project member size limit").into());
        }
        let mut bytes = Vec::new();
        (&mut entry).take(limit + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > limit {
            return Err(invalid("project member expansion limit").into());
        }
        Ok(bytes)
    };
    let manifest: Manifest = serde_json::from_slice(&read("manifest.json", 64_000)?)?;
    if manifest.format != "antivector" || manifest.version != 1 || !valid_name(&manifest.name) {
        return Err(invalid("unsupported project format/version").into());
    }
    av_core::raster::validate_dimensions(
        manifest.width,
        manifest.height,
        av_core::raster::DEFAULT_PIXEL_LIMIT,
    )?;
    let params: Params = serde_json::from_slice(&read("params.json", 64_000)?)?;
    params.validate()?;
    let edits: Vec<RecordedEdit> = serde_json::from_slice(&read("edits.json", EDIT_LIMIT)?)?;
    if edits.len() > 10_000 {
        return Err(invalid("too many project edits").into());
    }
    let png = read("original.png", IMAGE_LIMIT)?;
    if !png.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(invalid("project original must be PNG").into());
    }
    let image = decode::load_bytes(&png, av_core::raster::DEFAULT_PIXEL_LIMIT)?;
    if (image.w, image.h) != (manifest.width, manifest.height) {
        return Err(invalid("project image dimensions differ").into());
    }
    Ok(Project {
        image,
        params,
        edits,
        name: manifest.name,
    })
}
