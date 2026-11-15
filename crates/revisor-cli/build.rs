//! revisor-cli.exe gets the same Windows manifest as revisor.exe and its own icon and
//! VERSIONINFO, as `crates/revisor/build.rs` gives revisor.exe (TASK-47, ADR-0015).

use std::path::{Path, PathBuf};
use std::process::Command;

use embed_manifest::manifest::{ActiveCodePage, DpiAwareness, HeapType, Setting, SupportedOS};
use embed_manifest::{embed_manifest, new_manifest};

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // The same settings as revisor.exe's: UTF-8 as the active code page and long paths
        // matter to a program that takes paths from the command line too.
        let manifest = new_manifest("Revisor.CommandLine")
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

/// Compiles `resources/cli.rc` (the icon and VERSIONINFO) to a `.res` and hands it to the
/// linker. The icon is revisor.exe's, in the resources of the application crate, which the
/// include path makes findable; the identity is this exe's own. The manifest stays with
/// embed-manifest above, so the `.res` carries no RT_MANIFEST of its own.
fn embed_resources() {
    println!("cargo:rerun-if-changed=resources/cli.rc");
    println!("cargo:rerun-if-changed=../revisor/resources/app.ico");

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let resources = manifest_dir.join("resources");
    let app_resources = manifest_dir.join("..").join("revisor").join("resources");
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let res = out_dir.join("cli.res");
    let compiler = resource_compiler();
    let status = Command::new(&compiler)
        .arg("/I")
        .arg(&resources)
        .arg("/I")
        .arg(&app_resources)
        .arg("/FO")
        .arg(&res)
        .arg(resources.join("cli.rc"))
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

/// `llvm-rc` from the LLVM installation `LIBCLANG_PATH` points at, as in the application's
/// build.rs: the build of the workspace already needs that LLVM for wxdragon-sys's bindgen,
/// and the SDK's `rc.exe` is not on `PATH` outside a developer prompt.
fn resource_compiler() -> PathBuf {
    println!("cargo:rerun-if-env-changed=LIBCLANG_PATH");
    std::env::var_os("LIBCLANG_PATH")
        .map(|libclang| Path::new(&libclang).join("llvm-rc.exe"))
        .filter(|candidate| candidate.is_file())
        .unwrap_or_else(|| PathBuf::from("llvm-rc"))
}
