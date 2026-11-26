//! Connexion à Google (OAuth 2.0 « application de bureau » avec PKCE, redirection locale)
//! et relais des appels aux API Gmail et Google Agenda.
//!
//! Les jetons ne passent jamais par l'interface : l'interface demande à ce module
//! d'effectuer la requête, et ce module ajoute lui-même l'autorisation.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, State};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

/// Droits demandés à Google : Gmail (lire, étiqueter), Agenda (lecture seule) et Tasks (import des « Ajouter aux tâches »).
const SCOPES: &str = "https://www.googleapis.com/auth/gmail.modify https://www.googleapis.com/auth/calendar.readonly https://www.googleapis.com/auth/tasks";
const KEYRING_SERVICE: &str = "enviro-travail";
const KEYRING_USER: &str = "google-refresh-token";

#[derive(Default)]
pub struct AuthState {
    access: Mutex<Option<(String, Instant)>>,
}

#[derive(Serialize, Deserialize, Clone)]
struct ClientConfig {
    client_id: String,
    client_secret: String,
}

#[derive(Serialize)]
pub struct Status {
    configured: bool,
    connected: bool,
}

fn client_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("client.json"))
}

fn load_client(app: &AppHandle) -> Option<ClientConfig> {
    let path = client_path(app).ok()?;
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(|e| e.to_string())
}

fn stored_refresh() -> Option<String> {
    entry().ok()?.get_password().ok()
}

#[tauri::command]
pub fn google_status(app: AppHandle) -> Status {
    Status {
        configured: load_client(&app).is_some(),
        connected: stored_refresh().is_some(),
    }
}

/// Enregistre l'identifiant et le secret du client OAuth créé dans la console Google Cloud.
#[tauri::command]
pub fn save_client(app: AppHandle, client_id: String, client_secret: String) -> Result<(), String> {
    let cfg = ClientConfig {
        client_id: client_id.trim().to_string(),
        client_secret: client_secret.trim().to_string(),
    };
    if cfg.client_id.is_empty() || cfg.client_secret.is_empty() {
        return Err("Identifiant ou secret vide".to_string());
    }
    let json = serde_json::to_string(&cfg).map_err(|e| e.to_string())?;
    std::fs::write(client_path(&app)?, json).map_err(|e| e.to_string())
}

/// Efface l'identifiant et le secret enregistrés (pour en saisir de nouveaux).
#[tauri::command]
pub fn reset_client(app: AppHandle) -> Result<(), String> {
    let path = client_path(&app)?;
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn google_logout(state: State<'_, AuthState>) -> Result<(), String> {
    if let Ok(en) = entry() {
        let _ = en.delete_credential();
    }
    *state.access.lock().await = None;
    Ok(())
}

/// Ouvre une adresse https dans le navigateur par défaut.
#[tauri::command]
pub fn open_external(url: String) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("URL non autorisée".to_string());
    }
    webbrowser::open(&url).map_err(|e| e.to_string())
}

/// Attend la redirection de Google vers http://127.0.0.1:<port> et en extrait le code.
async fn wait_for_code(listener: TcpListener, expected_state: &str) -> Result<String, String> {
    loop {
        let (mut sock, _) = listener.accept().await.map_err(|e| e.to_string())?;
        let mut data = vec![0u8; 8192];
        let n = sock.read(&mut data).await.map_err(|e| e.to_string())?;
        let req = String::from_utf8_lossy(&data[..n]).to_string();
        let first = req.lines().next().unwrap_or("");
        let path = first.split_whitespace().nth(1).unwrap_or("/");
        let parsed = url::Url::parse(&format!("http://127.0.0.1{path}")).map_err(|e| e.to_string())?;

        let mut code: Option<String> = None;
        let mut state: Option<String> = None;
        let mut error: Option<String> = None;
        for (k, v) in parsed.query_pairs() {
            match k.as_ref() {
                "code" => code = Some(v.to_string()),
                "state" => state = Some(v.to_string()),
                "error" => error = Some(v.to_string()),
                _ => {}
            }
        }

        let body = "<html><head><meta charset=\"utf-8\"></head><body style=\"font-family:sans-serif;text-align:center;margin-top:20vh\"><h2>Connexion terminée</h2><p>Tu peux fermer cet onglet et revenir à l'application.</p></body></html>";
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = sock.write_all(resp.as_bytes()).await;
        let _ = sock.shutdown().await;

        if let Some(e) = error {
            return Err(format!("Google a refusé la connexion : {e}"));
        }
        if let (Some(c), Some(s)) = (code, state) {
            if s == expected_state {
                return Ok(c);
            }
        }
        // Autre requête (ex. favicon) : on continue d'attendre la bonne.
    }
}

#[tauri::command]
pub async fn google_login(app: AppHandle, state: State<'_, AuthState>) -> Result<(), String> {
    let cfg = load_client(&app).ok_or_else(|| "Identifiants Google non configurés".to_string())?;

    let listener = TcpListener::bind("127.0.0.1:0").await.map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect = format!("http://127.0.0.1:{port}");

    // PKCE : un secret aléatoire à usage unique, dont seule l'empreinte part chez Google.
    let mut raw = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut raw);
    let verifier = URL_SAFE_NO_PAD.encode(raw);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let mut raw_state = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut raw_state);
    let expected_state = URL_SAFE_NO_PAD.encode(raw_state);

    let mut auth = url::Url::parse("https://accounts.google.com/o/oauth2/v2/auth").map_err(|e| e.to_string())?;
    auth.query_pairs_mut()
        .append_pair("client_id", &cfg.client_id)
        .append_pair("redirect_uri", &redirect)
        .append_pair("response_type", "code")
        .append_pair("scope", SCOPES)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &expected_state)
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent");
    webbrowser::open(auth.as_str()).map_err(|e| e.to_string())?;

    let code = tokio::time::timeout(Duration::from_secs(180), wait_for_code(listener, &expected_state))
        .await
        .map_err(|_| "Connexion expirée (3 minutes). Recommence.".to_string())??;

    let http = reqwest::Client::new();
    let v: Value = http
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", cfg.client_id.as_str()),
            ("client_secret", cfg.client_secret.as_str()),
            ("code", code.as_str()),
            ("code_verifier", verifier.as_str()),
            ("redirect_uri", redirect.as_str()),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let refresh = v["refresh_token"]
        .as_str()
        .ok_or_else(|| format!("Réponse de Google inattendue : {v}"))?;
    entry()?.set_password(refresh).map_err(|e| e.to_string())?;

    let access = v["access_token"].as_str().unwrap_or_default().to_string();
    let lifetime = v["expires_in"].as_u64().unwrap_or(3000).saturating_sub(60);
    *state.access.lock().await = Some((access, Instant::now() + Duration::from_secs(lifetime)));
    Ok(())
}

/// Renvoie un jeton d'accès valide (rafraîchi si nécessaire).
async fn access_token(app: &AppHandle, state: &AuthState, force: bool) -> Result<String, String> {
    if !force {
        let cached = state.access.lock().await.clone();
        if let Some((token, expiry)) = cached {
            if Instant::now() < expiry {
                return Ok(token);
            }
        }
    }
    let cfg = load_client(app).ok_or_else(|| "Identifiants Google non configurés".to_string())?;
    let refresh = stored_refresh().ok_or_else(|| "Non connecté à Google".to_string())?;

    let http = reqwest::Client::new();
    let v: Value = http
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", cfg.client_id.as_str()),
            ("client_secret", cfg.client_secret.as_str()),
            ("refresh_token", refresh.as_str()),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    if let Some(token) = v["access_token"].as_str() {
        let lifetime = v["expires_in"].as_u64().unwrap_or(3000).saturating_sub(60);
        *state.access.lock().await = Some((token.to_string(), Instant::now() + Duration::from_secs(lifetime)));
        return Ok(token.to_string());
    }
    if v["error"].as_str() == Some("invalid_grant") {
        if let Ok(en) = entry() {
            let _ = en.delete_credential();
        }
        *state.access.lock().await = None;
        return Err("SESSION_EXPIRED".to_string());
    }
    Err(format!("Rafraîchissement refusé : {v}"))
}

/// Relais d'une requête vers Gmail ou Google Agenda, avec l'autorisation de l'utilisateur.
#[tauri::command]
pub async fn google_request(
    app: AppHandle,
    state: State<'_, AuthState>,
    method: String,
    url: String,
    body: Option<Value>,
) -> Result<Value, String> {
    let allowed = url.starts_with("https://gmail.googleapis.com/")
        || url.starts_with("https://www.googleapis.com/calendar/")
        || url.starts_with("https://tasks.googleapis.com/");
    if !allowed {
        return Err("URL non autorisée".to_string());
    }
    let http = reqwest::Client::new();
    let verb = reqwest::Method::from_bytes(method.as_bytes()).map_err(|e| e.to_string())?;
    let mut force = false;
    for _ in 0..2 {
        let token = access_token(&app, &state, force).await?;
        let mut req = http.request(verb.clone(), &url).bearer_auth(&token);
        if let Some(b) = &body {
            req = req.json(b);
        }
        let resp = req.send().await.map_err(|e| e.to_string())?;
        let status = resp.status();
        if status.as_u16() == 401 && !force {
            force = true;
            continue;
        }
        let text = resp.text().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(format!("Google {} : {}", status.as_u16(), text));
        }
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        return serde_json::from_str(&text).map_err(|e| e.to_string());
    }
    Err("Échec de l'authentification auprès de Google".to_string())
}
