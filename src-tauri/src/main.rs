// Évite d'ouvrir une console noire derrière l'application sous Windows (version finale).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod google;

use tauri::Emitter;
use tauri_plugin_updater::UpdaterExt;

/// Cherche une mise à jour ; si elle existe, la télécharge, l'installe puis relance l'application.
/// Appelée par l'interface au lancement (puis toutes les 6 heures) : le résultat est affiché à l'écran.
#[tauri::command]
async fn check_update(app: tauri::AppHandle) -> Result<String, String> {
    let current = app.package_info().version.to_string();
    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await.map_err(|e| e.to_string())? {
        Some(update) => {
            let _ = app.emit(
                "update-status",
                format!("Mise à jour {} : téléchargement et installation…", update.version),
            );
            update
                .download_and_install(|_chunk, _total| {}, || {})
                .await
                .map_err(|e| e.to_string())?;
            app.restart();
        }
        None => Ok(format!("Application à jour (version {current})")),
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(google::AuthState::default())
        .invoke_handler(tauri::generate_handler![
            google::google_status,
            google::save_client,
            google::reset_client,
            google::google_login,
            google::google_logout,
            google::google_request,
            google::open_external,
            check_update
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de l'application");
}
