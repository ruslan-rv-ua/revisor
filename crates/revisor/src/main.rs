// Hide the console window in release builds; keep it in debug for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use wxdragon::prelude::*;

fn main() {
    wxdragon::main(|_| {
        let frame = Frame::builder()
            .with_title(&format!("revisor {}", revisor_core::version()))
            .with_size(Size::new(800, 600))
            .build();
        frame.centre();
        frame.show(true);
    })
    .expect("failed to start the GUI");
}
