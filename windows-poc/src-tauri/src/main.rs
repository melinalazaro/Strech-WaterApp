#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#[cfg(target_os = "windows")]
mod position;

#[cfg(not(target_os = "windows"))]
fn main() { eprintln!("Esta prueba técnica requiere Windows."); }

#[cfg(target_os = "windows")]
fn main() {
    use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
    use std::time::Duration;
    use tauri::{Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder, menu::{Menu, MenuItem}, tray::TrayIconBuilder};
    use position::{Rect, Screen};
    #[repr(C)] struct Point { x: i32, y: i32 }
    #[repr(C)] struct WinRect { left: i32, top: i32, right: i32, bottom: i32 }
    #[repr(C)] struct MonitorInfo { size: u32, monitor: WinRect, work: WinRect, flags: u32 }
    #[link(name = "user32")]
    extern "system" {
        fn GetCursorPos(point: *mut Point) -> i32;
        fn MonitorFromPoint(point: Point, flags: u32) -> *mut std::ffi::c_void;
        fn GetMonitorInfoW(monitor: *mut std::ffi::c_void, info: *mut MonitorInfo) -> i32;
        fn ShowWindow(window: *mut std::ffi::c_void, command: i32) -> i32;
    }
    fn rect(r: &WinRect) -> Rect { Rect { left:r.left, top:r.top, right:r.right, bottom:r.bottom } }
    let generation = Arc::new(AtomicU64::new(0));
    tauri::Builder::default().setup(move |app| {
        let window = WebviewWindowBuilder::new(app, "reminder", WebviewUrl::App("index.html".into()))
            .title("SW PRUEBA").inner_size(290.0, 298.0).visible(false).focused(false)
            .focusable(false).transparent(true).decorations(false).shadow(false)
            .resizable(false).always_on_top(true).skip_taskbar(true).build()?;
        window.set_ignore_cursor_events(true)?;
        let test = MenuItem::with_id(app, "test", "Probar figura / Tester", true, None::<&str>)?;
        let quit = MenuItem::with_id(app, "quit", "Salir / Quitter", true, None::<&str>)?;
        let menu = Menu::with_items(app, &[&test, &quit])?;
        let counter = Arc::clone(&generation);
        TrayIconBuilder::new().icon(app.default_window_icon().unwrap().clone()).tooltip("SW PRUEBA")
            .menu(&menu).show_menu_on_left_click(true).on_menu_event(move |app, event| {
                if event.id.as_ref() == "quit" { app.exit(0); return; }
                if event.id.as_ref() != "test" { return; }
                let app2 = app.clone();
                let counter = Arc::clone(&counter);
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_secs(5));
                    let handle = app2.clone();
                    let _ = app2.run_on_main_thread(move || {
                let Some(window) = handle.get_webview_window("reminder") else { return; };
                let mut point = Point { x:0, y:0 };
                if unsafe { GetCursorPos(&mut point) } == 0 { return; }
                let monitor = unsafe { MonitorFromPoint(point, 1) };
                let mut info = MonitorInfo { size: std::mem::size_of::<MonitorInfo>() as u32, monitor: WinRect {left:0,top:0,right:0,bottom:0}, work: WinRect {left:0,top:0,right:0,bottom:0}, flags:0 };
                if monitor.is_null() || unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 { return; }
                let Ok(size) = window.outer_size() else { return; };
                let Ok(scale) = window.scale_factor() else { return; };
                let screens = [Screen { bounds:rect(&info.monitor), work:rect(&info.work) }];
                let Some((_, (x,y))) = position::position(&screens, (0,0), (size.width,size.height), (16.0*scale).round() as u32) else { return; };
                if window.set_position(PhysicalPosition::new(x,y)).is_err() { return; }
                let Ok(hwnd) = window.hwnd() else { return; };
                unsafe { ShowWindow(hwnd.0 as *mut std::ffi::c_void, 4); }
                let id = counter.fetch_add(1, Ordering::SeqCst)+1;
                let counter2 = Arc::clone(&counter);
                std::thread::spawn(move || { std::thread::sleep(Duration::from_secs(10)); if counter2.load(Ordering::SeqCst) == id { let _ = window.hide(); } });
                    });
                });
            }).build(app)?;
        Ok(())
    }).build(tauri::generate_context!()).expect("No se pudo iniciar la prueba")
    .run(|_, _| {});
}
