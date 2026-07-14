fn main() {
    println!("cargo:rerun-if-changed=launcher.rc");
    println!("cargo:rerun-if-changed=../desktop-tauri/icons/icon.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("launcher.rc", embed_resource::NONE)
            .manifest_required()
            .expect("failed to embed the AutoDesignMaker launcher icon");
    }
}
