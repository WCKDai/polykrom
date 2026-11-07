#![forbid(unsafe_code)]

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let icon = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/icons/app/windows/exports/polykrom-executable.ico");
    println!("cargo:rerun-if-changed={}", icon.display());
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS")? != "windows" {
        return Ok(());
    }
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").ok_or("missing OUT_DIR")?);
    let copy = out.join("polykrom.ico");
    std::fs::copy(icon, &copy)?;
    let rc = out.join("polykrom.rc");
    std::fs::write(&rc, format!("1 ICON \"{}\"\n", copy.display().to_string().replace('\\', "/")))?;
    embed_resource::compile_for(&rc, ["polykrom"], embed_resource::NONE).manifest_required()?;
    Ok(())
}
