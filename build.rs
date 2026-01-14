use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // Embed the Windows manifest that requests administrator privileges
    let _ = embed_resource::compile("gakumas-screenshot.rc", embed_resource::NONE);

    // Copy template folder to target directory
    copy_templates();
}

/// Copies the template folder to the target directory so the executable can find reference images.
fn copy_templates() {
    let out_dir = env::var("OUT_DIR").unwrap();
    // OUT_DIR is something like target/release/build/gakumas-screenshot-xxx/out
    // We need to go up to target/release (or target/debug)
    let out_path = Path::new(&out_dir);
    let target_dir = out_path
        .ancestors()
        .nth(3) // Go up 3 levels: out -> hash -> build -> release
        .expect("Could not find target directory");

    let template_src = Path::new("resources/template");
    let template_dst = target_dir.join("resources").join("template");

    if template_src.exists() {
        copy_dir_recursive(template_src, &template_dst);
        // Tell Cargo to re-run if templates change
        println!("cargo:rerun-if-changed=resources/template/");
    }
}

/// Recursively copies a directory and its contents.
fn copy_dir_recursive(src: &Path, dst: &Path) {
    let _ = fs::create_dir_all(dst);

    if let Ok(entries) = fs::read_dir(src) {
        for entry in entries.flatten() {
            let src_path = entry.path();
            let file_name = src_path.file_name().unwrap();
            let dst_path = dst.join(file_name);

            if src_path.is_dir() {
                copy_dir_recursive(&src_path, &dst_path);
            } else {
                let _ = fs::copy(&src_path, &dst_path);
            }
        }
    }
}
