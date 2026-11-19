#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod audio;
mod coordinator;
mod logging;
mod login;
mod menu;
#[cfg(test)]
mod phase3_tests;
mod power;
mod preferences;
mod presentation;
mod runtime;
mod scheduler;
use tauri::Manager;
fn main() {
    let (store, _instance_lock) = match preferences::Store::bundle() {
        Ok(preferences::Opened::Owner(store, lock)) => (store, lock),
        Ok(preferences::Opened::AlreadyRunning) => return,
        Err(e) => {
            power::startup_error(e.to_string());
            return;
        }
    };
    logging::initialize(&store);
    tauri::Builder::default()
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            runtime::start(app.handle().clone(), store)?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("No se pudo iniciar Stretch & Water")
        .run(|app, event| match event {
            tauri::RunEvent::Ready => {
                logging::record(&format!("Event loop Ready. PID={}", std::process::id()));
                if let Some(icon) = app.tray_by_id("stretch-water") {
                    if let Err(error) = icon.set_visible(true) {
                        logging::record(&format!("Error de visibilidad: {error}"));
                    }
                    logging::record(&format!("Geometría SW: {:?}", icon.rect()));
                }
            }
            tauri::RunEvent::ExitRequested { code, api, .. } => {
                if code.is_none()
                    && !app
                        .try_state::<runtime::Bridge>()
                        .is_some_and(|b| b.is_stopping())
                {
                    // Destroying a failed presentation must not close a tray utility.
                    api.prevent_exit();
                    logging::record("Salida implícita sin ventana impedida; usar Salir en SW.");
                } else {
                    if let Some(bridge) = app.try_state::<runtime::Bridge>() {
                        bridge.stop();
                    }
                    logging::record(&format!("Cierre solicitado: {code:?}"));
                }
            }
            tauri::RunEvent::Exit => {
                if let Some(bridge) = app.try_state::<runtime::Bridge>() {
                    bridge.stop();
                }
                logging::record("Proceso finalizando.");
            }
            _ => {}
        });
}
