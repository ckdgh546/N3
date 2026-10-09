const COMMANDS: &[&str] = &[
    "chatgpt_status",
    "chatgpt_sign_in",
    "chatgpt_translate",
    "chatgpt_sign_out",
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("failed to run tauri-build");
}
