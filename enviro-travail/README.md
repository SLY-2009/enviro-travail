# Enviro Travail — application Windows

Application de bureau (Rust + Tauri) : tes mails Gmail, tes tâches et ton agenda Google, dans une seule fenêtre.
Elle se met à jour toute seule à chaque lancement.

**Important : ce projet n'a pas pu être compilé ni testé avec un vrai compte Google par son auteur.**
L'interface a été vérifiée avec de fausses réponses de Google ; le code Rust, lui, n'a jamais été compilé.
Si la compilation affiche une erreur, copie le message tel quel et envoie-le à Claude pour correction.

---

## Étape 1 — Installer les outils sur ton PC Windows (une seule fois)

1. **Outils de compilation C++ de Microsoft** : https://visualstudio.microsoft.com/visual-cpp-build-tools/
   Lance l'installateur, coche **« Développement Desktop en C++ »**, installe.
2. **Rust** : https://rustup.rs → télécharge `rustup-init.exe`, accepte les choix par défaut. Ferme et rouvre PowerShell.
3. **WebView2** : déjà présent sur Windows 11 et sur Windows 10 à jour. Sinon : https://developer.microsoft.com/microsoft-edge/webview2/
4. **L'outil Tauri** (dans PowerShell) :
   ```powershell
   cargo install tauri-cli --version "^2" --locked
   ```
   (compte 5 à 15 minutes la première fois)

## Étape 2 — Autoriser l'application auprès de Google

C'est ce qui permet à l'application de parler à Gmail et à ton agenda.

1. Va sur https://console.cloud.google.com et connecte-toi (un compte Google personnel suffit).
2. **Nouveau projet** (menu en haut) → nom : « Enviro Travail ».
3. Menu **API et services → Bibliothèque** : active **Gmail API** puis **Google Calendar API**.
4. **API et services → Écran de consentement OAuth** :
   - Type d'utilisateur : **Externe** → Créer. Renseigne le nom de l'application et ton e-mail.
   - Dans **Utilisateurs test**, ajoute **l'adresse Google de ton école** (celle dont tu veux lire les mails).
   - Dans **Accès aux données / Niveaux d'accès**, ajoute :
     `.../auth/gmail.modify` et `.../auth/calendar.readonly`.
5. **API et services → Identifiants → Créer des identifiants → ID client OAuth** :
   - Type d'application : **Application de bureau** → Créer.
   - Note l'**ID client** et le **code secret du client**. Tu les colleras dans l'application au premier lancement.

**Deux points à savoir :**
- Si ton école utilise Google Workspace, son administrateur peut **bloquer** les applications non approuvées : Google affichera alors « Accès bloqué ». Il faut lui demander d'autoriser l'ID client de cette application
  (Console d'administration → Sécurité → Contrôle des accès et des données → Contrôles des API → Gérer l'accès des applications tierces).
  Tant que ce n'est pas fait, la connexion avec le compte de l'école échouera.
- Tant que l'écran de consentement est en mode **Test**, Google peut t'imposer de te **reconnecter environ une fois par semaine**.
  Le passer en **« En production »** supprime en principe cette limite, mais affiche l'avertissement « application non vérifiée » (à passer). À vérifier dans la documentation Google au moment où tu le fais.

## Étape 3 — Essayer l'application

Dans PowerShell, dans le dossier du projet :
```powershell
cd chemin\vers\enviro-travail
cargo tauri dev
```
La première compilation est longue (10 à 20 minutes). Une fenêtre s'ouvre : colle l'ID client et le code secret, clique **Enregistrer**, puis **Se connecter à Google**.
Ton navigateur s'ouvre pour te connecter à ton compte et accepter les droits ; reviens ensuite à l'application.
(La mise à jour automatique affichera une erreur dans la console tant que l'étape 4 n'est pas faite : c'est normal.)

## Étape 4 — Mise à jour automatique : créer ta clé de signature et ton dépôt GitHub

Les mises à jour sont **signées** : l'application refuse toute mise à jour qui n'a pas ta signature.

1. **Créer la clé** (PowerShell) :
   ```powershell
   cargo tauri signer generate -w "$HOME\.tauri\enviro.key"
   ```
   Choisis un mot de passe (ou laisse vide). Deux fichiers sont créés : `enviro.key` (privé, à ne jamais partager) et `enviro.key.pub` (public).
2. Ouvre `enviro.key.pub`, copie tout son contenu et colle-le dans `src-tauri/tauri.conf.json`, à la place de `COLLE_ICI_LA_CLE_PUBLIQUE...` (champ `pubkey`).
3. Crée un compte sur https://github.com, puis un dépôt **public** nommé `enviro-travail` (public pour que l'application puisse lire les mises à jour sans identifiant).
4. Dans `src-tauri/tauri.conf.json`, remplace `VOTRE-COMPTE-GITHUB` par ton nom d'utilisateur GitHub dans `endpoints`.
5. Sur GitHub : **Settings → Secrets and variables → Actions → New repository secret** :
   - `TAURI_SIGNING_PRIVATE_KEY` = le **contenu** du fichier `enviro.key`
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` = le mot de passe choisi (même vide, crée le secret)
6. Envoie le projet sur GitHub (installe Git for Windows si besoin) :
   ```powershell
   git init
   git add .
   git commit -m "Première version"
   git branch -M main
   git remote add origin https://github.com/TON-COMPTE/enviro-travail.git
   git push -u origin main
   ```

## Étape 5 — Publier la première version et l'installer

```powershell
git tag v0.1.0
git push origin v0.1.0
```
GitHub construit l'installateur (compte 15 à 25 minutes). Dans l'onglet **Actions** du dépôt, suis la progression.
Quand c'est terminé, va dans **Releases**, télécharge le fichier **`Enviro Travail_0.1.0_x64-setup.exe`** et lance-le. L'application s'installe pour ton utilisateur (pas besoin d'être administrateur).

> Windows SmartScreen peut afficher « Windows a protégé votre ordinateur » car l'installateur n'est pas signé par un éditeur reconnu :
> clique **Informations complémentaires → Exécuter quand même**.

## Étape 6 — Publier une mise à jour

1. Modifie le code, puis augmente le numéro de version **dans `src-tauri/tauri.conf.json`** (champ `version`) **et dans `src-tauri/Cargo.toml`** : par ex. `0.1.1`.
2. ```powershell
   git add .
   git commit -m "Version 0.1.1"
   git push
   git tag v0.1.1
   git push origin v0.1.1
   ```
3. Au prochain lancement, l'application détecte la nouvelle version, la télécharge, l'installe et se relance toute seule.

## Où sont tes données ?

- **Jetons Google** : dans le Gestionnaire d'identification de Windows (jamais en clair dans un fichier).
- **ID client / code secret** : `%APPDATA%\fr.artfx.envirotravail\client.json`
- **Tâches, disposition, thème** : dans la mémoire locale de l'application, sur ce PC. Elles ne se synchronisent pas avec la version web.
- Pour te déconnecter ou changer d'ID client : icône ⚙ en haut de la fenêtre.

## Droits demandés à Google

- Gmail (`gmail.modify`) : lire tes mails, les étiqueter, les archiver, les marquer comme lus, enregistrer des brouillons et envoyer des réponses. Pas de suppression définitive.
- Agenda (`calendar.readonly`) : lecture seule.

## Différences avec la version web

- Les mails sont lus via l'API Gmail ; l'adresse du compte sert à ouvrir les bons liens Gmail et Agenda.
- La signature est celle de Gmail (réglée dans Gmail) ; l'application l'ajoute à la fin de la réponse. Le message cité n'est pas repris dans la réponse.
- Les notifications des tâches urgentes sont des notifications Windows.
- Les liens s'ouvrent dans ton navigateur par défaut.

## Si quelque chose ne marche pas

- **Erreur à la compilation** : copie le message complet et envoie-le à Claude.
- **« Accès bloqué » ou « application non autorisée » chez Google** : voir les « deux points à savoir » de l'étape 2.
- **La mise à jour ne se fait pas** : vérifie la clé publique, l'adresse `endpoints`, que le dépôt est public et que la Release contient un fichier `latest.json`.
