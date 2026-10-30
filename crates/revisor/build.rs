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
    }
    println!("cargo:rerun-if-changed=build.rs");
}
