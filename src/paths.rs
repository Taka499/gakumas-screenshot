use std::path::PathBuf;
use std::sync::OnceLock;

static EXE_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Returns the directory containing the executable.
pub fn get_exe_dir() -> &'static PathBuf {
    EXE_DIR.get_or_init(|| {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."))
    })
}

/// Returns the logs directory: `<exe_dir>/logs/`
pub fn get_logs_dir() -> PathBuf {
    get_exe_dir().join("logs")
}

/// Returns the screenshots directory: `<exe_dir>/screenshots/`
pub fn get_screenshots_dir() -> PathBuf {
    get_exe_dir().join("screenshots")
}

/// Returns the rehearsal template directory: `<exe_dir>/resources/template/rehearsal/`
pub fn get_rehearsal_template_dir() -> PathBuf {
    get_exe_dir().join("resources").join("template").join("rehearsal")
}

/// Returns the tesseract directory: `<exe_dir>/tesseract/`
pub fn get_tesseract_dir() -> PathBuf {
    get_exe_dir().join("tesseract")
}

/// Ensures all output directories exist. Call at startup.
pub fn ensure_directories() -> std::io::Result<()> {
    std::fs::create_dir_all(get_logs_dir())?;
    std::fs::create_dir_all(get_screenshots_dir())?;
    std::fs::create_dir_all(get_rehearsal_template_dir())?;
    Ok(())
}
