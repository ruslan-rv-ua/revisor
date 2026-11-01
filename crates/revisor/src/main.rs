// Hide the console window in release builds; keep it in debug for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use wxdragon::prelude::*;

fn main() {
    wxdragon::main(|_| {
        let frame = Frame::builder()
            .with_title(&format!("revisor {}", revisor_core::version()))
            .with_size(Size::new(800, 600))
            .build();
        set_frame_icon(&frame);
        frame.centre();
        frame.show(true);
    })
    .expect("failed to start the GUI");
}

/// Gives the window the same icon Explorer shows for the exe. wxMSW does not adopt the
/// exe's icon resource for a frame, so the frame takes it from the SVG that app.ico is
/// generated from. A failure leaves the generic window icon, which is not worth failing over.
fn set_frame_icon(frame: &Frame) {
    const ICON: &[u8] = include_bytes!("../resources/icon.svg");
    if let Some(bitmap) = BitmapBundle::from_svg_data(ICON, Size::new(32, 32))
        .and_then(|bundle| bundle.get_bitmap_for(frame))
    {
        frame.set_icon(&bitmap);
    }
}
