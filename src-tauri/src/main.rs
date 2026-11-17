// Évite d'ouvrir une console noire derrière l'application sous Windows (version finale).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod google;

use tauri::Emitter;
use tauri_plugin_updater::UpdaterExt;

/// Vérifie au lancement s'il existe une version plus récente ; si oui, la télécharge,
/// l'installe puis relance l'application.
async fn check_update(app: tauri::AppHandle) -> Result<(), String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    if let Some(update) = updater.check().await.map_err(|e| e.to_string())? {
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
    Ok(())
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
            google::open_external
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = check_update(handle).await {
                    eprintln!("Vérification de mise à jour : {e}");
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("erreur au lancement de l'application");
}
