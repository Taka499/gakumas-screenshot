fn main() {
    // Embed the Windows manifest that requests administrator privileges
    let _ = embed_resource::compile("gakumas-screenshot.rc", embed_resource::NONE);
}
