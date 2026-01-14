use anyhow::{anyhow, Result};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use crate::log;

const TESSERACT_VERSION: &str = "5.5.0";
const TESSDATA_REPO: &str = "https://github.com/tesseract-ocr/tessdata/raw/main";

pub struct TesseractPaths {
    pub executable: PathBuf,
    pub tessdata: PathBuf,
}

/// Returns the directory for storing Tesseract files
pub fn get_tesseract_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gakumas-screenshot")
        .join("tesseract")
}

/// Ensures Tesseract is installed. Downloads if necessary.
pub fn ensure_tesseract() -> Result<TesseractPaths> {
    let tesseract_dir = get_tesseract_dir();
    let executable = tesseract_dir.join("tesseract.exe");
    let tessdata_dir = tesseract_dir.join("tessdata");
    let eng_traineddata = tessdata_dir.join("eng.traineddata");

    // Check if already installed
    if executable.exists() && eng_traineddata.exists() {
        log(&format!("Tesseract found at: {}", tesseract_dir.display()));
        return Ok(TesseractPaths {
            executable,
            tessdata: tessdata_dir,
        });
    }

    log("Tesseract not found locally, downloading...");

    // Create directories
    fs::create_dir_all(&tesseract_dir)?;
    fs::create_dir_all(&tessdata_dir)?;

    // Download Tesseract executable and DLLs
    if !executable.exists() {
        download_tesseract(&tesseract_dir)?;
    }

    // Download English trained data
    if !eng_traineddata.exists() {
        download_tessdata(&tessdata_dir)?;
    }

    log(&format!(
        "Tesseract ready at: {}",
        tesseract_dir.display()
    ));

    Ok(TesseractPaths {
        executable,
        tessdata: tessdata_dir,
    })
}

/// Downloads Tesseract executable and required DLLs from UB-Mannheim releases
fn download_tesseract(tesseract_dir: &PathBuf) -> Result<()> {
    // UB-Mannheim provides Windows builds with all dependencies
    // We'll download the portable zip version
    let download_url = format!(
        "https://github.com/UB-Mannheim/tesseract/releases/download/v{}/tesseract-ocr-w64-setup-{}.exe",
        TESSERACT_VERSION, TESSERACT_VERSION
    );

    log(&format!("Note: Tesseract auto-download from UB-Mannheim is complex."));
    log(&format!("For now, please manually install Tesseract:"));
    log(&format!("1. Download from: https://github.com/UB-Mannheim/tesseract/releases"));
    log(&format!("2. Install to default location or add to PATH"));
    log(&format!("3. Or copy tesseract.exe and DLLs to: {}", tesseract_dir.display()));

    // Check if Tesseract is in PATH
    if let Ok(output) = std::process::Command::new("tesseract")
        .arg("--version")
        .output()
    {
        if output.status.success() {
            log("Found Tesseract in system PATH, will use that instead.");
            // Create a batch file that calls system tesseract
            let batch_content = "@echo off\r\ntesseract %*\r\n";
            fs::write(tesseract_dir.join("tesseract.exe"), batch_content)?;
            return Ok(());
        }
    }

    // Try common installation paths
    let common_paths = [
        r"C:\Program Files\Tesseract-OCR\tesseract.exe",
        r"C:\Program Files (x86)\Tesseract-OCR\tesseract.exe",
    ];

    for path in &common_paths {
        if PathBuf::from(path).exists() {
            log(&format!("Found Tesseract at: {}", path));
            // Copy to our directory
            let exe_path = tesseract_dir.join("tesseract.exe");
            // Create a batch file to redirect to the installed version
            let batch_content = format!("@echo off\r\n\"{}\" %*\r\n", path);
            fs::write(&exe_path, batch_content)?;
            return Ok(());
        }
    }

    // If we reach here, we need to actually download
    // For now, try downloading from a more accessible source
    download_tesseract_portable(tesseract_dir)
}

/// Attempts to download a portable Tesseract build
fn download_tesseract_portable(tesseract_dir: &PathBuf) -> Result<()> {
    // Alternative: Use the zip archive from digi4.navi-it.net or other sources
    // For simplicity, we'll try to download from a direct link

    // First, try to find if there's a local copy
    let local_tesseract = PathBuf::from("tesseract.exe");
    if local_tesseract.exists() {
        fs::copy(&local_tesseract, tesseract_dir.join("tesseract.exe"))?;
        return Ok(());
    }

    Err(anyhow!(
        "Could not find or download Tesseract. Please install Tesseract-OCR manually:\n\
         1. Download from: https://github.com/UB-Mannheim/tesseract/releases\n\
         2. Run the installer (choose to add to PATH)\n\
         3. Or extract portable version to: {}\n\
         4. Restart this application after installation",
        tesseract_dir.display()
    ))
}

/// Downloads English trained data
fn download_tessdata(tessdata_dir: &PathBuf) -> Result<()> {
    let eng_url = format!("{}/eng.traineddata", TESSDATA_REPO);
    let eng_path = tessdata_dir.join("eng.traineddata");

    log("Downloading eng.traineddata...");

    // Check if system tessdata exists
    let system_tessdata_paths = [
        r"C:\Program Files\Tesseract-OCR\tessdata\eng.traineddata",
        r"C:\Program Files (x86)\Tesseract-OCR\tessdata\eng.traineddata",
    ];

    for path in &system_tessdata_paths {
        if PathBuf::from(path).exists() {
            log(&format!("Copying eng.traineddata from: {}", path));
            fs::copy(path, &eng_path)?;
            return Ok(());
        }
    }

    // Download from GitHub
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()?;

    let response = client
        .get(&eng_url)
        .header("User-Agent", "gakumas-screenshot")
        .send()?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to download eng.traineddata: HTTP {}",
            response.status()
        ));
    }

    let bytes = response.bytes()?;
    let mut file = fs::File::create(&eng_path)?;
    file.write_all(&bytes)?;

    log(&format!(
        "Downloaded eng.traineddata ({} bytes)",
        bytes.len()
    ));

    Ok(())
}

/// Finds the Tesseract executable, checking our local dir first, then system
pub fn find_tesseract_executable() -> Result<PathBuf> {
    let tesseract_dir = get_tesseract_dir();
    let local_exe = tesseract_dir.join("tesseract.exe");

    if local_exe.exists() {
        return Ok(local_exe);
    }

    // Check PATH
    if let Ok(output) = std::process::Command::new("tesseract")
        .arg("--version")
        .output()
    {
        if output.status.success() {
            return Ok(PathBuf::from("tesseract"));
        }
    }

    // Check common paths
    let common_paths = [
        r"C:\Program Files\Tesseract-OCR\tesseract.exe",
        r"C:\Program Files (x86)\Tesseract-OCR\tesseract.exe",
    ];

    for path in &common_paths {
        let p = PathBuf::from(path);
        if p.exists() {
            return Ok(p);
        }
    }

    Err(anyhow!("Tesseract not found. Please install Tesseract-OCR."))
}

/// Finds the tessdata directory
pub fn find_tessdata_dir() -> Result<PathBuf> {
    let tesseract_dir = get_tesseract_dir();
    let local_tessdata = tesseract_dir.join("tessdata");

    if local_tessdata.join("eng.traineddata").exists() {
        return Ok(local_tessdata);
    }

    // Check system paths
    let system_paths = [
        r"C:\Program Files\Tesseract-OCR\tessdata",
        r"C:\Program Files (x86)\Tesseract-OCR\tessdata",
    ];

    for path in &system_paths {
        let p = PathBuf::from(path);
        if p.join("eng.traineddata").exists() {
            return Ok(p);
        }
    }

    // Check TESSDATA_PREFIX environment variable
    if let Ok(prefix) = std::env::var("TESSDATA_PREFIX") {
        let p = PathBuf::from(&prefix);
        if p.join("eng.traineddata").exists() {
            return Ok(p);
        }
        let p = PathBuf::from(&prefix).join("tessdata");
        if p.join("eng.traineddata").exists() {
            return Ok(p);
        }
    }

    Err(anyhow!(
        "tessdata directory not found. Please ensure eng.traineddata is available."
    ))
}
