/* Pont entre l'interface et Google : reproduit, avec les API Gmail et Agenda,
   les « outils » que la version web obtenait des connecteurs de claude.ai.
   Les jetons restent dans la partie Rust ; ici, on ne fait que demander des requêtes. */
(() => {
"use strict";
const T = window.__TAURI__;
if (!T) return; // ouvert hors de l'application : on ne touche à rien

const inv = (c, a) => T.core.invoke(c, a);
let resolveReady; const ready = new Promise(r => { resolveReady = r; });

/* ---------- Fenêtre de connexion ---------- */
const el = (t, st, txt, ch) => { const e = document.createElement(t); if (st) e.style.cssText = st; if (txt != null) e.textContent = txt; (ch || []).forEach(c => e.append(c)); return e; };
let gateEl = null;
const closeGate = () => { if (gateEl) { gateEl.remove(); gateEl = null; } };
async function showGate(msg) {
  closeGate();
  let st = { configured: false, connected: false };
  try { st = await inv("google_status"); } catch (e) {}
  const box = el("div", "max-width:480px;width:92%;background:var(--card,#fff);color:var(--tx,#1c1c1a);border:1px solid var(--bd,#dcdad2);border-radius:14px;padding:22px;box-shadow:0 10px 40px rgba(0,0,0,.25)");
  gateEl = el("div", "position:fixed;inset:0;z-index:99999;background:rgba(0,0,0,.4);display:flex;align-items:center;justify-content:center;font:14px system-ui,sans-serif", null, [box]);
  box.append(el("h2", "margin:0 0 10px;font-size:18px", "Connexion à Google"));
  if (msg) box.append(el("p", "color:#c0392b;margin:0 0 10px", msg));
  const err = el("p", "color:#c0392b;min-height:1.2em;margin:8px 0 0");
  const inStyle = "width:100%;padding:7px;margin:4px 0 10px;box-sizing:border-box;border:1px solid var(--bd,#ccc);border-radius:6px;background:var(--card,#fff);color:inherit";
  const btn = (txt, fn, primary) => { const b = el("button", "padding:7px 14px;border-radius:8px;border:1px solid var(--bd,#ccc);cursor:pointer;margin-right:8px;" + (primary ? "background:#2f5d50;color:#fff;border-color:#2f5d50" : "background:var(--card,#fff);color:inherit"), txt); b.onclick = async () => { b.disabled = true; try { await fn(); } catch (e) { err.textContent = String(e && e.message || e); b.disabled = false; } }; return b; };
  if (!st.configured) {
    box.append(el("p", "margin:0 0 10px", "Colle l'identifiant et le secret du client Google créé dans la console Google Cloud (voir le guide, étape 2). Ils restent sur cet ordinateur."));
    const id = el("input", inStyle), sec = el("input", inStyle);
    id.placeholder = "ID client (xxxx.apps.googleusercontent.com)"; sec.placeholder = "Code secret du client"; sec.type = "password";
    box.append(id, sec, btn("Enregistrer", async () => { await inv("save_client", { clientId: id.value, clientSecret: sec.value }); await showGate(); }, true));
  } else if (!st.connected) {
    box.append(el("p", "margin:0 0 10px", "Un onglet de ton navigateur va s'ouvrir : connecte-toi avec ton compte Google de l'école et accepte les droits demandés."));
    box.append(btn("Se connecter à Google", async () => { await inv("google_login"); closeGate(); resolveReady(); if (window.refreshAll) window.refreshAll(); }, true));
    box.append(btn("Changer les identifiants", async () => { try { await inv("google_logout"); } catch (e) {} await inv("reset_client"); await showGate(); }));
  } else {
    box.append(el("p", "margin:0 0 10px", "Tu es connecté à Google."));
    box.append(btn("Fermer", async () => closeGate(), true), btn("Se déconnecter", async () => { await inv("google_logout"); await showGate(); }));
  }
  box.append(err);
  document.body.append(gateEl);
}
const needLogin = () => showGate("Ta session Google a expiré. Reconnecte-toi.");

/* ---------- Aides Gmail ---------- */
const G = "https://gmail.googleapis.com/gmail/v1/users/me/";
const CAL = "https://www.googleapis.com/calendar/v3/calendars/primary/events";
const gapi = (method, url, body) => inv("google_request", { method, url, body: body === undefined ? null : body });
let me = "";
async function myEmail() { if (!me) { const p = await gapi("GET", G + "profile"); me = p.emailAddress || ""; } return me; }
const threadUrl = (em, id) => "https://mail.google.com/mail/?authuser=" + encodeURIComponent(em) + "#all/" + id;
const hdr = (m, n) => { const h = ((m.payload && m.payload.headers) || []).find(x => x.name.toLowerCase() === n.toLowerCase()); return h ? h.value : ""; };
function addrs(v) {
  const out = [], re = /(?:"?([^"<,]*?)"?\s*)?<([^>]+)>|([^\s,<>]+@[^\s,<>]+)/g; let m;
  while ((m = re.exec(v || ""))) { const email = (m[2] || m[3] || "").trim(); if (email) out.push({ name: (m[1] || "").trim(), email }); }
  return out;
}
const b64d = s => { const bin = atob(s.replace(/-/g, "+").replace(/_/g, "/")); return new TextDecoder().decode(Uint8Array.from(bin, c => c.charCodeAt(0))); };
function collect(p, out) { if (!p) return; if (p.mimeType && p.body && p.body.data) out.push({ t: p.mimeType, d: p.body.data }); (p.parts || []).forEach(x => collect(x, out)); }
function bodyText(p) {
  const o = []; collect(p, o);
  const t = o.find(x => x.t === "text/plain"); if (t) return b64d(t.d);
  const h = o.find(x => x.t === "text/html");
  if (h) { const d = new DOMParser().parseFromString(b64d(h.d), "text/html"); d.querySelectorAll("style,script").forEach(e => e.remove()); return ((d.body && (d.body.innerText || d.body.textContent)) || "").trim(); }
  return "";
}
function mapMsg(m, tid, withBody) {
  const from = addrs(hdr(m, "From"))[0] || { email: "", name: "" }, to = addrs(hdr(m, "To")), cc = addrs(hdr(m, "Cc"));
  const names = [from, ...to, ...cc].filter(a => a.name).map(a => ({ emailAddress: a.email, displayName: a.name }));
  const o = { id: m.id, threadId: tid, subject: hdr(m, "Subject"), sender: from.email, date: m.internalDate ? new Date(+m.internalDate).toISOString() : "", snippet: m.snippet || "", labelIds: m.labelIds || [], toRecipients: to.map(a => a.email), ccRecipients: cc.map(a => a.email), addressDisplayNames: names };
  if (withBody) o.plaintextBody = bodyText(m.payload);
  return o;
}

/* ---------- « Outils » Gmail ---------- */
async function searchThreads(a) {
  const n = a.pageSize || 25; let q = a.query || "in:inbox", lab = "";
  const lm = /label:(\S+)/.exec(q); // « label:à-répondre » : on retrouve l'identifiant de l'étiquette par son nom
  if (lm) {
    const norm = x => x.toLowerCase().replace(/\s+/g, "-");
    const r = await gapi("GET", G + "labels");
    const lb = (r.labels || []).find(x => norm(x.name) === norm(lm[1]));
    if (!lb) return { threads: [] };
    lab = "&labelIds=" + encodeURIComponent(lb.id); q = q.replace(lm[0], "").trim();
  }
  const l = await gapi("GET", G + "threads?maxResults=" + n + lab + (q ? "&q=" + encodeURIComponent(q) : ""));
  const ids = (l.threads || []).map(t => t.id), em = await myEmail(), out = []; let i = 0;
  const hs = "&metadataHeaders=Subject&metadataHeaders=From&metadataHeaders=To&metadataHeaders=Cc&metadataHeaders=Date";
  async function worker() { while (i < ids.length) { const k = i++; const t = await gapi("GET", G + "threads/" + ids[k] + "?format=metadata" + hs); out[k] = { id: t.id, messages: (t.messages || []).map(m => mapMsg(m, t.id, false)), viewUrl: threadUrl(em, t.id) }; } }
  await Promise.all(Array.from({ length: 6 }, worker));
  return { threads: out.filter(Boolean) };
}
async function getThread(a) {
  const t = await gapi("GET", G + "threads/" + a.threadId + "?format=full"), em = await myEmail();
  return { id: t.id, viewUrl: threadUrl(em, t.id), messages: (t.messages || []).map(m => mapMsg(m, t.id, true)) };
}
async function listLabels() {
  const r = await gapi("GET", G + "labels");
  return { labels: (r.labels || []).map(l => ({ labelId: l.id, name: l.name, labelType: l.type === "user" ? "USER" : "SYSTEM", color: l.color || null })) };
}
async function createLabel(a) {
  const l = await gapi("POST", G + "labels", { name: a.name, labelListVisibility: "labelShow", messageListVisibility: "show" });
  return { labelId: l.id, name: l.name, labelType: "USER" };
}
async function modify(a, add, rem) { await gapi("POST", G + "threads/" + a.threadId + "/modify", { addLabelIds: add || [], removeLabelIds: rem || [] }); return {}; }

/* ---------- Réponse / brouillon (message MIME construit ici) ---------- */
const enc = s => { let b = ""; new TextEncoder().encode(s).forEach(c => { b += String.fromCharCode(c); }); return btoa(b); };
const wrap76 = s => s.replace(/.{1,76}/g, "$&\r\n").trimEnd();
const b64u = s => s.replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
const esc = s => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
async function signature() { try { const r = await gapi("GET", G + "settings/sendAs"); const p = (r.sendAs || []).find(x => x.isPrimary) || (r.sendAs || [])[0]; return (p && p.signature) || ""; } catch (e) { return ""; } }
async function buildReply(a, replyAll) {
  const mid0 = a.messageId || a.replyToMessageId;
  const m = await gapi("GET", G + "messages/" + mid0 + "?format=metadata&metadataHeaders=Message-ID&metadataHeaders=References&metadataHeaders=Subject&metadataHeaders=From&metadataHeaders=To&metadataHeaders=Cc&metadataHeaders=Reply-To");
  const em = (await myEmail()).toLowerCase();
  const from = addrs(hdr(m, "From"))[0] || { email: "" };
  const rt = addrs(hdr(m, "Reply-To"))[0] || from;
  let to = [rt.email], cc = [];
  if (from.email.toLowerCase() === em) to = addrs(hdr(m, "To")).map(x => x.email); // dernier message envoyé par moi
  if (replyAll) cc = [...new Set([...addrs(hdr(m, "To")), ...addrs(hdr(m, "Cc"))].map(x => x.email).filter(e => e.toLowerCase() !== em && !to.includes(e)))];
  let subj = hdr(m, "Subject"); if (!/^re:/i.test(subj)) subj = "Re: " + subj;
  const id = hdr(m, "Message-ID"), refs = (hdr(m, "References") + " " + id).trim();
  let html = "<div>" + esc(a.body || "").replace(/\r?\n/g, "<br>") + "</div>";
  if (a.includeSignature) { const s = await signature(); if (s) html += "<br><div>" + s + "</div>"; }
  const raw = ["To: " + to.join(", "), cc.length ? "Cc: " + cc.join(", ") : null, "Subject: =?UTF-8?B?" + enc(subj) + "?=", id ? "In-Reply-To: " + id : null, "References: " + refs, "MIME-Version: 1.0", "Content-Type: text/html; charset=UTF-8", "Content-Transfer-Encoding: base64", "", wrap76(enc("<html><body>" + html + "</body></html>"))].filter(x => x !== null).join("\r\n");
  return { raw: b64u(btoa(raw)), threadId: m.threadId };
}
async function sendReply(a) { const msg = await buildReply(a, !!a.replyAll); await gapi("POST", G + "messages/send", msg); return {}; }
async function createDraft(a) { const msg = await buildReply(a, false); const d = await gapi("POST", G + "drafts", { message: msg }); return d || {}; }

/* ---------- Agenda ---------- */
async function listEvents(a) {
  const now = new Date(), sod = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const t0 = a.startTime ? new Date(a.startTime) : sod, t1 = a.endTime ? new Date(a.endTime) : new Date(+sod + 7 * 864e5);
  const q = new URLSearchParams({ singleEvents: "true", orderBy: "startTime", timeMin: t0.toISOString(), timeMax: t1.toISOString(), maxResults: String(Math.min(a.pageSize || 30, 250)) });
  if (a.pageToken) q.set("pageToken", a.pageToken);
  const r = await gapi("GET", CAL + "?" + q.toString());
  return { summary: r.summary, events: r.items || [], nextPageToken: r.nextPageToken };
}

/* ---------- Adaptateur « connecteurs » attendu par l'interface ---------- */
const TOOLS = {
  "Gmail": { search_threads: searchThreads, get_thread: getThread, list_labels: listLabels, create_label: createLabel,
    label_thread: a => modify(a, a.labelIds, []), unlabel_thread: a => modify(a, [], a.labelIds),
    reply: sendReply, create_draft: createDraft },
  "Google Calendar": { list_events: listEvents }
};
async function call(server, tool, args) {
  await ready;
  try {
    const f = (TOOLS[server] || {})[tool]; if (!f) throw new Error("Outil non pris en charge : " + tool);
    return { payload: await f(args || {}) };
  } catch (e) {
    const msg = String((e && e.message) || e);
    if (/SESSION_EXPIRED|Non connecté/.test(msg)) { needLogin(); throw { code: "needs_reauth", message: msg }; }
    throw { code: "tool_error", message: msg };
  }
}
function watchTool(server, tool, args, handler, opts) {
  let stop = false;
  const run = async () => { if (stop) return; try { handler({ type: "data", result: await call(server, tool, args) }); } catch (e) { handler({ type: "error", error: { code: e.code || "tool_error", message: e.message } }); } };
  run();
  const iv = opts && opts.refetchInterval ? setInterval(run, opts.refetchInterval) : null;
  return () => { stop = true; if (iv) clearInterval(iv); };
}
const mcp = { callTool: (s, t, a) => call(s, t, a), watchTool, invalidate: async () => {} };
window.claude = { use: async name => name === "mcp" ? (await ready, mcp) : null };

/* ---------- Notifications Windows, liens, mises à jour ---------- */
const notif = T.notification;
if (notif) {
  try { notif.isPermissionGranted().then(ok => { if (!ok) notif.requestPermission(); }); } catch (e) {}
  window.Notification = class { constructor(title, o) { try { notif.sendNotification({ title, body: (o && o.body) || "" }); } catch (e) {} } static get permission() { return "granted"; } static requestPermission() { return Promise.resolve("granted"); } };
}
document.addEventListener("click", e => {
  const a = e.target.closest && e.target.closest("a[href]"); if (!a) return;
  if (/^https:\/\//i.test(a.href)) { e.preventDefault(); inv("open_external", { url: a.href }).catch(() => {}); }
}, true);
if (T.event) T.event.listen("update-status", ev => { if (window.toast) window.toast(String(ev.payload)); });

const gear = el("button", "border:0;background:none;font-size:15px;cursor:pointer;padding:2px 6px", "⚙");
gear.title = "Compte Google"; gear.onclick = () => showGate();
const mb = document.querySelector("#mb"); if (mb) mb.insertBefore(gear, document.querySelector("#bell"));
inv("google_status").then(s => { if (s.configured && s.connected) resolveReady(); else showGate(); }).catch(() => showGate());

/* ---------- Mise à jour : au lancement (résultat affiché), puis toutes les 6 heures ---------- */
const say = m => { if (window.toast) window.toast(m); };
const checkUpdate = quiet => inv("check_update").then(m => { if (!quiet) say(m); }).catch(e => { if (!quiet) say("Mise à jour impossible : " + String(e).slice(0, 160)); });
setTimeout(() => checkUpdate(false), 2500);
setInterval(() => checkUpdate(true), 6 * 3600 * 1000);
})();
