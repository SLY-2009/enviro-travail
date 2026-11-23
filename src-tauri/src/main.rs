// Évite d'ouvrir une console noire derrière l'application sous Windows (version finale).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod google;

use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

const UI_URL: &str = "https://raw.githubusercontent.com/SLY-2009/enviro-travail/main/ui/app.html";

async fn fetch_ui(url: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(6))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    resp.text().await.map_err(|e| e.to_string())
}

/// Charge l'interface : la dernière version publiée sur GitHub si elle est joignable,
/// sinon la copie gardée en cache lors du dernier lancement.
#[tauri::command]
async fn load_ui(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let cache = dir.join("ui-cache.html");
    let previous = std::fs::read_to_string(&cache).ok();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    match fetch_ui(&format!("{UI_URL}?t={stamp}")).await {
        Ok(html) if html.contains("name=\"enviro-ui\"") => {
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(&cache, &html);
            let updated = previous.as_deref() != Some(html.as_str());
            Ok(serde_json::json!({ "html": html, "updated": updated, "source": "reseau" }))
        }
        _ => match previous {
            Some(html) => Ok(serde_json::json!({ "html": html, "updated": false, "source": "cache" })),
            None => Err("Aucune interface distante ni en cache".to_string()),
        },
    }
}

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
            check_update,
            load_ui
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de l'application");
}
