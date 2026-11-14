use std::path::{Path, PathBuf};
use std::process::Command;

use embed_manifest::manifest::{ActiveCodePage, DpiAwareness, HeapType, Setting, SupportedOS};
use embed_manifest::{embed_manifest, new_manifest};

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // new_manifest already enables Common Controls v6 (native theming); the settings
        // below are spelled out so the choices are visible, long paths per the brief.
        let manifest = new_manifest("Revisor")
            .supported_os(SupportedOS::Windows7..=SupportedOS::Windows10)
            .active_code_page(ActiveCodePage::Utf8)
            .heap_type(HeapType::SegmentHeap)
            .dpi_awareness(DpiAwareness::PerMonitorV2)
            .long_path_aware(Setting::Enabled);
        embed_manifest(manifest).expect("failed to embed the Windows manifest");
        embed_resources();
    }
    println!("cargo:rerun-if-changed=build.rs");
}

/// Compiles `resources/app.rc` (the icon and VERSIONINFO) to a `.res` and hands it to the
/// linker. The manifest stays with embed-manifest above, which passes it to the linker with
/// `/MANIFEST:EMBED`, so the `.res` carries no RT_MANIFEST of its own.
fn embed_resources() {
    println!("cargo:rerun-if-changed=resources/app.rc");
    println!("cargo:rerun-if-changed=resources/app.ico");

    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources");
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let res = out_dir.join("app.res");
    let compiler = resource_compiler();
    let status = Command::new(&compiler)
        // The .rc names its icon relatively; the include path makes it findable from
        // wherever cargo runs this.
        .arg("/I")
        .arg(&resources)
        .arg("/FO")
        .arg(&res)
        .arg(resources.join("app.rc"))
        .status()
        .unwrap_or_else(|e| {
            panic!(
                "{} could not be run: {e}; set LIBCLANG_PATH to LLVM's bin folder",
                compiler.display()
            )
        });
    assert!(status.success(), "{} failed: {status}", compiler.display());

    println!("cargo:rustc-link-arg-bins={}", res.display());
}

/// `llvm-rc` from the LLVM installation `LIBCLANG_PATH` points at. The build already needs
/// that LLVM for wxdragon-sys's bindgen, so the resource compiler adds no new requirement;
/// the SDK's `rc.exe` is not on `PATH` outside a developer prompt.
fn resource_compiler() -> PathBuf {
    println!("cargo:rerun-if-env-changed=LIBCLANG_PATH");
    std::env::var_os("LIBCLANG_PATH")
        .map(|libclang| Path::new(&libclang).join("llvm-rc.exe"))
        .filter(|candidate| candidate.is_file())
        .unwrap_or_else(|| PathBuf::from("llvm-rc"))
}
