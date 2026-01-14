use anyhow::{anyhow, Result};
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use zip::ZipArchive;

use crate::log;

/// Embedded Tesseract package (tesseract.exe + DLLs + tessdata)
const TESSERACT_ZIP: &[u8] = include_bytes!("../../resources/tesseract.zip");

pub struct TesseractPaths {
    pub executable: PathBuf,
    pub tessdata: PathBuf,
}

/// Ensures Tesseract is available. Extracts from embedded zip if necessary.
pub fn ensure_tesseract() -> Result<TesseractPaths> {
    let tesseract_dir = crate::paths::get_tesseract_dir();
    let executable = tesseract_dir.join("tesseract.exe");
    let tessdata_dir = tesseract_dir.join("tessdata");
    let eng_traineddata = tessdata_dir.join("eng.traineddata");

    // Check if already extracted
    if executable.exists() && eng_traineddata.exists() {
        log(&format!("Tesseract found at: {}", tesseract_dir.display()));
        return Ok(TesseractPaths {
            executable,
            tessdata: tessdata_dir,
        });
    }

    log("Extracting embedded Tesseract...");
    extract_embedded_tesseract(&tesseract_dir)?;

    // Verify extraction succeeded
    if !executable.exists() {
        return Err(anyhow!(
            "Tesseract extraction failed: tesseract.exe not found at {}",
            executable.display()
        ));
    }
    if !eng_traineddata.exists() {
        return Err(anyhow!(
            "Tesseract extraction failed: eng.traineddata not found at {}",
            eng_traineddata.display()
        ));
    }

    log(&format!(
        "Tesseract extracted to: {}",
        tesseract_dir.display()
    ));

    Ok(TesseractPaths {
        executable,
        tessdata: tessdata_dir,
    })
}

/// Extracts the embedded Tesseract zip to the target directory
fn extract_embedded_tesseract(target_dir: &Path) -> Result<()> {
    let cursor = Cursor::new(TESSERACT_ZIP);
    let mut archive = ZipArchive::new(cursor)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let raw_name = file.name().to_string();

        // Handle both forward and backslash paths (zip may have either)
        let normalized_name = raw_name.replace('\\', "/");
        let outpath = target_dir.join(&normalized_name);

        if normalized_name.ends_with('/') {
            // Directory entry
            fs::create_dir_all(&outpath)?;
        } else {
            // File entry
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut outfile = fs::File::create(&outpath)?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            std::io::Write::write_all(&mut outfile, &buffer)?;
        }
    }

    Ok(())
}

/// Finds the Tesseract executable (only checks local extraction)
pub fn find_tesseract_executable() -> Result<PathBuf> {
    let tesseract_dir = crate::paths::get_tesseract_dir();
    let local_exe = tesseract_dir.join("tesseract.exe");

    if local_exe.exists() {
        return Ok(local_exe);
    }

    Err(anyhow!(
        "Tesseract not found at {}. Run ensure_tesseract() first.",
        local_exe.display()
    ))
}

/// Finds the tessdata directory (only checks local extraction)
pub fn find_tessdata_dir() -> Result<PathBuf> {
    let tesseract_dir = crate::paths::get_tesseract_dir();
    let local_tessdata = tesseract_dir.join("tessdata");

    if local_tessdata.join("eng.traineddata").exists() {
        return Ok(local_tessdata);
    }

    Err(anyhow!(
        "tessdata not found at {}. Run ensure_tesseract() first.",
        local_tessdata.display()
    ))
}
