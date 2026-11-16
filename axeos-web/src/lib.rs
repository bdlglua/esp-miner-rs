//! Browser UI written in Rust. JavaScript is limited to wasm-bindgen's generated loader.
use serde_json::{json, Value};
use std::{cell::RefCell, fmt::Write};
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{Document, Element, Event, HtmlInputElement, RequestInit, Response};

#[derive(Default)]
struct App {
    info: Option<Value>,
    history: Vec<Value>,
    logs: Vec<Value>,
    page: String,
    busy: bool,
    mutation: bool,
    online: bool,
    revision: u64,
    redraw: bool,
    network_error: bool,
}
thread_local! {static APP:RefCell<App>=RefCell::new(App{page:"dashboard".into(),..App::default()});}
fn document() -> Document {
    web_sys::window().unwrap().document().unwrap()
}
fn element(id: &str) -> Option<Element> {
    document().get_element_by_id(id)
}
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn text(value: &Value, path: &str) -> String {
    escape(value.pointer(path).and_then(Value::as_str).unwrap_or("—"))
}
fn flag(value: &Value, path: &str) -> bool {
    value
        .pointer(path)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}
fn num(value: &Value, path: &str, digits: usize) -> String {
    value
        .pointer(path)
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite())
        .map(|n| format!("{n:.digits$}"))
        .unwrap_or_else(|| "—".into())
}
fn uptime(seconds: u64) -> String {
    format!(
        "{} j {:02} h {:02} min",
        seconds / 86400,
        seconds / 3600 % 24,
        seconds / 60 % 60
    )
}
fn icon(name: &str) -> String {
    let path=match name {
        "dashboard"=>"<rect x='3' y='3' width='7' height='7' rx='1'/><rect x='14' y='3' width='7' height='7' rx='1'/><rect x='3' y='14' width='7' height='7' rx='1'/><rect x='14' y='14' width='7' height='7' rx='1'/>",
        "pool"=>"<path d='M4 6h16v5H4zM4 15h16v5H4zM8 8.5h.1M8 17.5h.1'/>",
        "hardware"=>"<rect x='6' y='6' width='12' height='12' rx='2'/><path d='M9 2v4m6-4v4M9 18v4m6-4v4M2 9h4m-4 6h4m12-6h4m-4 6h4'/>",
        "network"=>"<path d='M3 8a15 15 0 0 1 18 0M6 12a10 10 0 0 1 12 0m-9 4a5 5 0 0 1 6 0'/><circle cx='12' cy='20' r='1'/>",
        "logs"=>"<path d='m4 6 4 4-4 4m7 3h9'/>",
        _=>"<circle cx='12' cy='12' r='9'/><path d='M12 8v5m0 3h.01'/>",
    };
    format!("<svg viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round' aria-hidden='true'>{path}</svg>")
}
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let window = web_sys::window().unwrap();
    let theme = window
        .local_storage()
        .ok()
        .flatten()
        .and_then(|s| s.get_item("axeos-theme").ok().flatten())
        .unwrap_or_else(|| "dark".into());
    document().document_element().unwrap().set_attribute(
        "data-theme",
        if theme == "light" { "light" } else { "dark" },
    )?;
    let mut nav = String::new();
    for (page, label) in [
        ("dashboard", "Vue d’ensemble"),
        ("pool", "Pool"),
        ("hardware", "Matériel"),
        ("network", "Réseau"),
        ("logs", "Journaux"),
        ("system", "Système"),
    ] {
        let _ = write!(
            nav,
            "<button type='button' data-page='{page}'>{}<span>{label}</span></button>",
            icon(page)
        );
    }
    element("app").unwrap().set_inner_html(&format!(r##"
        <aside class="sidebar"><a class="brand" href="#dashboard"><span class="brand-mark">₿</span><span>AxeOS <small>Rust</small></span></a>
        <div class="device-tag"><span class="status-dot"></span><span id="device-name">Bitaxe</span><small>ESP32-S3</small></div>
        <nav aria-label="Navigation principale">{nav}</nav><div class="sidebar-bottom"><button type="button" data-action="theme">Changer de thème</button><p>Rust · WebAssembly<br>Interface embarquée</p></div></aside>
        <div class="workspace"><header class="topbar"><span id="connection" role="status">Connexion au mineur…</span><span id="device-ip" class="mono"></span></header>
        <div id="notice" role="status" class="notice" hidden></div><main id="content"><div class="empty">Chargement des données du mineur…</div></main>
        <footer>AxeOS Rust <span>Exécuté dans votre navigateur · Données du mineur local</span></footer></div>"##));
    let click = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
        let Some(target) = event.target().and_then(|t| t.dyn_into::<Element>().ok()) else {
            return;
        };
        if let Ok(Some(button)) = target.closest("[data-page]") {
            if let Some(page) = button.get_attribute("data-page") {
                let _ = web_sys::window().unwrap().location().set_hash(&page);
                navigate(&page);
            }
        } else if let Ok(Some(button)) = target.closest("[data-action]") {
            if button.has_attribute("disabled") {
                return;
            }
            match button.get_attribute("data-action").as_deref() {
                Some("theme") => toggle_theme(),
                Some("restart") => {
                    if web_sys::window()
                        .unwrap()
                        .confirm_with_message(
                            "Redémarrer le mineur ? La connexion sera momentanément interrompue.",
                        )
                        .unwrap_or(false)
                    {
                        mutate("POST", "/api/system/restart", json!({}));
                    }
                }
                Some("pause") => mutate("POST", "/api/system/pause", json!({})),
                Some("resume") => mutate("POST", "/api/system/resume", json!({})),
                _ => {}
            }
        }
    });
    document().add_event_listener_with_callback("click", click.as_ref().unchecked_ref())?;
    click.forget();
    let submit = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
        let Some(form) = event.target().and_then(|t| t.dyn_into::<Element>().ok()) else {
            return;
        };
        let Some(kind) = form.get_attribute("data-form") else {
            return;
        };
        event.prevent_default();
        match form_patch(&kind) {
            Ok(patch) => mutate("PATCH", "/api/system", patch),
            Err(e) => notice(&e, true),
        }
    });
    document().add_event_listener_with_callback("submit", submit.as_ref().unchecked_ref())?;
    submit.forget();
    let hash = Closure::<dyn FnMut(Event)>::new(move |_| load_hash());
    window.add_event_listener_with_callback("hashchange", hash.as_ref().unchecked_ref())?;
    hash.forget();
    load_hash();
    refresh();
    let timer = Closure::<dyn FnMut()>::new(refresh);
    window.set_interval_with_callback_and_timeout_and_arguments_0(
        timer.as_ref().unchecked_ref(),
        3000,
    )?;
    timer.forget();
    Ok(())
}
fn load_hash() {
    let hash = web_sys::window()
        .unwrap()
        .location()
        .hash()
        .unwrap_or_default();
    navigate(hash.trim_start_matches('#'));
}
fn navigate(page: &str) {
    let page = match page {
        "pool" | "hardware" | "network" | "logs" | "system" => page,
        _ => "dashboard",
    };
    APP.with(|s| s.borrow_mut().page = page.into());
    render();
}
fn toggle_theme() {
    let root = document().document_element().unwrap();
    let light = root.get_attribute("data-theme").as_deref() != Some("light");
    let theme = if light { "light" } else { "dark" };
    let _ = root.set_attribute("data-theme", theme);
    if let Ok(Some(storage)) = web_sys::window().unwrap().local_storage() {
        let _ = storage.set_item("axeos-theme", theme);
    }
}
async fn api(method: &str, path: &str, body: Option<Value>) -> Result<Value, String> {
    let window = web_sys::window().ok_or("Navigateur indisponible")?;
    let init = RequestInit::new();
    init.set_method(method);
    init.set_mode(web_sys::RequestMode::SameOrigin);
    let headers = web_sys::Headers::new().map_err(|_| "En-têtes indisponibles")?;
    if let Some(body) = body {
        headers
            .set("Content-Type", "application/json")
            .map_err(|_| "En-tête invalide")?;
        init.set_body(&JsValue::from_str(&body.to_string()));
    }
    init.set_headers(&headers);
    let controller = web_sys::AbortController::new().map_err(|_| "Annulation indisponible")?;
    init.set_signal(Some(&controller.signal()));
    let abort = controller.clone();
    let timeout = Closure::once(move || abort.abort());
    let handle = window
        .set_timeout_with_callback_and_timeout_and_arguments_0(
            timeout.as_ref().unchecked_ref(),
            10000,
        )
        .map_err(|_| "Délai indisponible")?;
    let result = async {
        let response = JsFuture::from(window.fetch_with_str_and_init(path, &init))
            .await
            .map_err(|_| "Mineur injoignable. Vérifiez la connexion et son adresse IP.")?
            .dyn_into::<Response>()
            .map_err(|_| "Réponse HTTP invalide")?;
        let raw = JsFuture::from(response.text().map_err(|_| "Réponse illisible")?)
            .await
            .map_err(|_| "Réponse interrompue")?
            .as_string()
            .ok_or("Réponse vide")?;
        let value: Value =
            serde_json::from_str(&raw).map_err(|_| "Le serveur n’a pas renvoyé du JSON valide")?;
        if !response.ok() {
            return Err(value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("Échec de la requête")
                .into());
        }
        Ok(value)
    }
    .await;
    window.clear_timeout_with_handle(handle);
    drop(timeout);
    result
}
fn refresh() {
    let token = APP.with(|s| {
        let mut s = s.borrow_mut();
        if s.busy || s.mutation {
            None
        } else {
            s.busy = true;
            Some(s.revision)
        }
    });
    let Some(token) = token else {
        return;
    };
    spawn_local(async move {
        let result = api("GET", "/api/system/info", None).await;
        match result {
            Ok(info) => {
                let history = api("GET", "/api/system/statistics", None).await.ok();
                let logs = api("GET", "/api/system/events", None).await.ok();
                let stale = APP.with(|s| s.borrow().revision != token);
                if stale {
                    APP.with(|s| s.borrow_mut().busy = false);
                    refresh();
                    return;
                }
                let (redraw, clear_error) = APP.with(|s| {
                    let mut s = s.borrow_mut();
                    let first = s.info.is_none();
                    s.info = Some(info);
                    s.online = true;
                    if let Some(v) = history {
                        s.history = v.as_array().cloned().unwrap_or_default();
                    }
                    if let Some(v) = logs {
                        s.logs = v.as_array().cloned().unwrap_or_default();
                    }
                    s.busy = false;
                    let redraw = first
                        || s.redraw
                        || matches!(s.page.as_str(), "dashboard" | "system" | "logs");
                    let clear = s.network_error;
                    s.network_error = false;
                    s.redraw = false;
                    (redraw, clear)
                });
                if clear_error {
                    if let Some(e) = element("notice") {
                        let _ = e.set_attribute("hidden", "");
                    }
                }
                update_header();
                if redraw {
                    render();
                }
            }
            Err(message) => {
                let stale = APP.with(|s| s.borrow().revision != token);
                APP.with(|s| {
                    let mut s = s.borrow_mut();
                    s.busy = false;
                    if !stale {
                        s.online = false;
                        s.network_error = true;
                    }
                });
                if stale {
                    refresh();
                    return;
                }
                update_header();
                notice(&message, true);
            }
        }
    });
}
fn update_header() {
    APP.with(|s| {
        let s = s.borrow();
        if let Some(info) = &s.info {
            if let Some(e) = element("device-name") {
                e.set_text_content(Some(&format!(
                    "Bitaxe {} · {}",
                    info["deviceModel"].as_str().unwrap_or(""),
                    info["boardVersion"].as_str().unwrap_or("")
                )));
            }
            if let Some(e) = element("device-ip") {
                e.set_text_content(info["ipv4"].as_str());
            }
            if let Some(e) = element("connection") {
                e.set_text_content(Some(if s.online {
                    "Connecté au mineur · actualisation toutes les 3 s"
                } else {
                    "Connexion perdue · données figées"
                }));
                let _ = e.set_attribute("class", if s.online { "online" } else { "offline" });
            }
        }
    });
}
fn notice(message: &str, error: bool) {
    if let Some(e) = element("notice") {
        e.set_text_content(Some(message));
        let _ = e.remove_attribute("hidden");
        let _ = e.set_attribute(
            "class",
            if error {
                "notice error"
            } else {
                "notice success"
            },
        );
    }
}
fn mutate(method: &'static str, path: &'static str, body: Value) {
    let run = APP.with(|s| {
        let mut s = s.borrow_mut();
        if s.mutation {
            false
        } else {
            s.mutation = true;
            s.revision = s.revision.wrapping_add(1);
            s.network_error = false;
            true
        }
    });
    if !run {
        return;
    }
    notice("Envoi au mineur…", false);
    if let Ok(buttons) = document().query_selector_all("button[type=submit]") {
        for i in 0..buttons.length() {
            if let Some(node) = buttons.item(i) {
                if let Ok(e) = node.dyn_into::<Element>() {
                    let _ = e.set_attribute("disabled", "");
                }
            }
        }
    }
    spawn_local(async move {
        match api(method, path, Some(body)).await {
            Ok(value) => {
                APP.with(|s| s.borrow_mut().redraw = true);
                let mut message = value["message"]
                    .as_str()
                    .unwrap_or("Opération terminée.")
                    .to_string();
                if flag(&value, "/restartRequired") {
                    message.push_str(" Redémarrez le mineur pour appliquer les changements.");
                }
                notice(&message, false);
                if path == "/api/system" {
                    for id in ["wifi-password", "pool-password"] {
                        if let Ok(input) = input(id) {
                            input.set_value("");
                        }
                    }
                }
            }
            Err(message) => notice(&message, true),
        }
        APP.with(|s| s.borrow_mut().mutation = false);
        if let Ok(buttons) = document().query_selector_all("button[type=submit]") {
            for i in 0..buttons.length() {
                if let Some(node) = buttons.item(i) {
                    if let Ok(e) = node.dyn_into::<Element>() {
                        let _ = e.remove_attribute("disabled");
                    }
                }
            }
        }
        refresh();
    });
}
fn input(id: &str) -> Result<HtmlInputElement, String> {
    element(id)
        .and_then(|e| e.dyn_into().ok())
        .ok_or_else(|| "Champ de formulaire introuvable".into())
}
fn val(id: &str) -> Result<String, String> {
    Ok(input(id)?.value())
}
fn checked(id: &str) -> Result<bool, String> {
    Ok(input(id)?.checked())
}
fn number(id: &str) -> Result<u64, String> {
    val(id)?
        .parse()
        .map_err(|_| "Valeur numérique invalide".into())
}
fn form_patch(kind: &str) -> Result<Value, String> {
    Ok(match kind {
        "pool" => {
            let mut p = json!({"poolHost":val("pool-host")?,"poolPort":number("pool-port")?,"poolUser":val("pool-user")?});
            let secret = val("pool-password")?;
            if !secret.is_empty() {
                p["poolPassword"] = json!(secret);
            }
            p
        }
        "hardware" => {
            json!({"frequency":number("frequency")?,"coreVoltage":number("core-voltage")?,"autoFan":checked("auto-fan")?,"fanPercent":number("fan-percent")?})
        }
        "network" => {
            let mut p = json!({"hostname":val("hostname")?,"ssid":val("ssid")?});
            let secret = val("wifi-password")?;
            if checked("open-network")? {
                p["wifiPassword"] = json!("");
            } else if !secret.is_empty() {
                p["wifiPassword"] = json!(secret);
            }
            p
        }
        _ => return Err("Formulaire inconnu".into()),
    })
}
fn render() {
    APP.with(|s|{
        let s=s.borrow();
        for page in ["dashboard","pool","hardware","network","logs","system"] {
            if let Ok(Some(e))=document().query_selector(&format!("nav [data-page='{page}']")){let _=e.set_attribute("class",if s.page==page{"active"}else{""});if s.page==page{let _=e.set_attribute("aria-current","page");}else{let _=e.remove_attribute("aria-current");}}
        }
        let Some(info)=&s.info else{return;};
        let body=match s.page.as_str(){"pool"=>pool(info),"hardware"=>hardware(info),"network"=>network(info),"logs"=>logs(&s.logs),"system"=>system(info),_=>dashboard(info,&s.history)};
        let banner=if flag(info,"/restartRequired"){r#"<div class="pending"><span>Des réglages enregistrés attendent un redémarrage.</span><button data-action="restart">Redémarrer</button></div>"#}else{""};
        if let Some(e)=element("content"){e.set_inner_html(&format!("{banner}{body}"));}
    });
}
fn heading(title: &str, description: &str) -> String {
    format!("<div class='page-heading'><div><h1>{title}</h1><p>{description}</p></div></div>")
}
fn metric(label: &str, value: String, unit: &str, detail: &str) -> String {
    format!("<article class='metric'><span>{label}</span><div class='metric-value'>{value}<small>{unit}</small></div><p>{detail}</p></article>")
}
fn row(label: &str, value: String) -> String {
    format!("<div class='data-row'><span>{label}</span><strong>{value}</strong></div>")
}
fn dashboard(info: &Value, history: &[Value]) -> String {
    let mut out = heading("Vue d’ensemble", "Votre Bitaxe, en direct.");
    let paused = flag(info, "/telemetry/miningPaused");
    let fault = flag(info, "/telemetry/fault");
    let status = if fault {
        "Défaut matériel · redémarrage requis après contrôle"
    } else if paused {
        "Minage en pause"
    } else if flag(info, "/telemetry/poolConnected") {
        "Pool connecté · worker autorisé"
    } else {
        "En attente de connexion au pool"
    };
    let _=write!(out,"<div class='mining-strip'><span class='{}'>{status}</span><button data-action='{}' {}>{}</button></div>",if fault{"danger"}else{"muted"},if paused{"resume"}else{"pause"},if fault{"disabled"}else{""},if paused{"Reprendre le minage"}else{"Mettre en pause"});
    if info
        .pointer("/settings/poolUser")
        .and_then(Value::as_str)
        .unwrap_or("")
        .is_empty()
    {
        out.push_str("<div class='pending'><span>Ajoutez votre adresse de minage et votre worker pour démarrer.</span><button data-page='pool'>Configurer le pool</button></div>");
    }
    out.push_str("<section class='metrics' aria-label='Indicateurs du mineur'>");
    out.push_str(&metric(
        "Débit estimé",
        num(info, "/telemetry/hashRate", 1),
        "GH/s",
        "Estimation sur les nonces des 60 dernières secondes",
    ));
    out.push_str(&metric(
        if info["deviceModel"] == "Gamma" {
            "Puissance ASIC"
        } else {
            "Puissance d’entrée"
        },
        num(info, "/telemetry/power", 1),
        "W",
        "Mesure du régulateur ou du moniteur d’alimentation",
    ));
    out.push_str(&metric(
        "Température ASIC",
        num(info, "/telemetry/temp", 1),
        "°C",
        "Protection thermique au-dessus de 75 °C",
    ));
    out.push_str(&metric(
        "Shares acceptées",
        num(info, "/telemetry/sharesAccepted", 0),
        "",
        &format!(
            "{} rejetées par le pool",
            num(info, "/telemetry/sharesRejected", 0)
        ),
    ));
    out.push_str("</section>");
    let _=write!(out,"<section class='panel chart-panel'><div class='panel-heading'><div><h2>Activité du mineur</h2><p>Débit estimé · GH/s</p></div><span class='pill'>5 dernières minutes</span></div>{}</section>",chart(history));
    out.push_str("<div class='two-columns'><section class='panel'><h2>Matériel</h2>");
    out.push_str(&row("ASIC", text(info, "/asicModel")));
    out.push_str(&row(
        "Fréquence active",
        format!("{} MHz", num(info, "/telemetry/frequency", 0)),
    ));
    out.push_str(&row(
        "Débit théorique",
        format!("{} GH/s", num(info, "/telemetry/expectedHashrate", 0)),
    ));
    out.push_str(&row(
        "Tension réelle du cœur",
        format!("{} mV", num(info, "/telemetry/coreVoltageActual", 0)),
    ));
    out.push_str(&row(
        "Ventilateur",
        format!(
            "{} % · {} tr/min",
            num(info, "/telemetry/fanPercent", 0),
            num(info, "/telemetry/fanRpm", 0)
        ),
    ));
    out.push_str("</section><section class='panel'><h2>Session de minage</h2>");
    out.push_str(&row("Pool", text(info, "/settings/poolHost")));
    out.push_str(&row(
        "Difficulté du pool",
        num(info, "/telemetry/poolDifficulty", 0),
    ));
    out.push_str(&row(
        "Meilleure difficulté",
        num(info, "/telemetry/bestDiff", 2),
    ));
    out.push_str(&row(
        "Nonces valides / invalides",
        format!(
            "{} / {}",
            num(info, "/telemetry/noncesValid", 0),
            num(info, "/telemetry/noncesInvalid", 0)
        ),
    ));
    out.push_str(&row(
        "Durée de fonctionnement",
        uptime(info["uptimeSeconds"].as_u64().unwrap_or(0)),
    ));
    out.push_str("</section></div>");
    out
}
fn chart(history: &[Value]) -> String {
    let count = history
        .iter()
        .filter(|s| s["hashRate"].as_f64().is_some())
        .count();
    if count < 2 {
        return "<div class='chart-empty'>La courbe apparaîtra après les premières mesures.<br><small>Aucune donnée simulée n’est affichée.</small></div>".into();
    }
    let max = history
        .iter()
        .filter_map(|s| s["hashRate"].as_f64())
        .fold(1.0f64, f64::max)
        * 1.15;
    let mut path = String::new();
    let mut drawing = false;
    for (i, s) in history.iter().enumerate() {
        if let Some(v) = s["hashRate"].as_f64() {
            let x = 48.0 + i as f64 * 900.0 / (history.len().max(2) - 1) as f64;
            let y = 180.0 - v / max * 150.0;
            let _ = write!(path, "{}{x:.1},{y:.1} ", if drawing { "L" } else { "M" });
            drawing = true;
        } else {
            drawing = false;
        }
    }
    format!("<svg class='chart' viewBox='0 0 980 220' role='img' aria-label='Historique du débit de hachage estimé'><path class='grid-line' d='M48 30H950M48 105H950M48 180H950'/><text x='4' y='35'>{max:.0}</text><text x='20' y='184'>0</text><text x='48' y='211'>Il y a 5 min au maximum</text><text x='875' y='211'>Maintenant</text><path class='hash-line' d='{path}'/></svg>")
}
fn field(id: &str, label: &str, kind: &str, value: String, attrs: &str) -> String {
    format!(
        "<label for='{id}'>{label}<input id='{id}' type='{kind}' value='{value}' {attrs}></label>"
    )
}
fn pool(info: &Value) -> String {
    let mut out = heading(
        "Pool",
        "Configurez votre pool Stratum V1 et votre adresse de minage.",
    );
    out.push_str("<section class='panel form-panel'><form data-form='pool'><h2>Connexion Stratum</h2><div class='form-grid'>");
    out.push_str(&field(
        "pool-host",
        "Hôte du pool",
        "text",
        text(info, "/settings/poolHost"),
        "required maxlength='128' placeholder='public-pool.io' autocomplete='off'",
    ));
    out.push_str(&field(
        "pool-port",
        "Port",
        "number",
        num(info, "/settings/poolPort", 0),
        "required min='1' max='65535'",
    ));
    out.push_str(&field(
        "pool-user",
        "Adresse de minage et worker",
        "text",
        text(info, "/settings/poolUser"),
        "required maxlength='64' placeholder='votre_adresse.gamma601' autocomplete='off'",
    ));
    out.push_str(&field("pool-password","Mot de passe du pool","password",String::new(),"maxlength='64' placeholder='Vide : conserver le mot de passe actuel' autocomplete='new-password'"));
    out.push_str("</div><p class='help'>Utilisez un nom DNS ou une IPv4, sans stratum+tcp://. Connexion TCP Stratum V1 ; TLS et pool de secours ne sont pas inclus dans ce port.</p><div class='form-actions'><p>Application après redémarrage.</p><button class='primary' type='submit'>Enregistrer le pool</button></div></form></section>");
    out
}
fn hardware(info: &Value) -> String {
    let mut out = heading(
        "Matériel",
        "Réglez l’ASIC et la ventilation dans les limites de votre carte.",
    );
    let _=write!(out,"<section class='panel form-panel'><form data-form='hardware'><h2>{} · Bitaxe {}</h2><div class='form-grid'>",text(info,"/asicModel"),text(info,"/deviceModel"));
    out.push_str(&field(
        "frequency",
        "Fréquence (MHz)",
        "number",
        num(info, "/settings/frequency", 0),
        "required min='100' max='525' step='1'",
    ));
    out.push_str(&field(
        "core-voltage",
        "Tension du cœur (mV)",
        "number",
        num(info, "/settings/coreVoltage", 0),
        &format!(
            "required min='{}' max='{}' step='1'",
            num(info, "/voltageMin", 0),
            num(info, "/voltageMax", 0)
        ),
    ));
    out.push_str(&field(
        "fan-percent",
        "Ventilation manuelle (%)",
        "number",
        num(info, "/settings/fanPercent", 0),
        "required min='30' max='100'",
    ));
    let _=write!(out,"<label class='checkbox'><input id='auto-fan' type='checkbox' {}><span>Ventilation automatique</span></label>",if flag(info,"/settings/autoFan"){"checked"}else{""});
    out.push_str("</div><p class='help'>La ventilation s’applique au prochain cycle de contrôle, sous 5 secondes. À partir de 70 °C, la protection impose 100 %. Fréquence et tension s’appliquent après redémarrage.</p><div class='form-actions'><p>Protection thermique maintenue à 75 °C.</p><button class='primary' type='submit'>Enregistrer les réglages</button></div></form></section>");
    out
}
fn network(info: &Value) -> String {
    let mut out = heading("Réseau", "Connexion Wi-Fi du mineur.");
    out.push_str("<section class='panel form-panel'><form data-form='network'><h2>Réseau Wi-Fi</h2><div class='form-grid'>");
    out.push_str(&field(
        "hostname",
        "Nom du mineur",
        "text",
        text(info, "/settings/hostname"),
        "required maxlength='32' pattern='[A-Za-z0-9-]+'",
    ));
    out.push_str(&field(
        "ssid",
        "SSID",
        "text",
        text(info, "/settings/ssid"),
        "required maxlength='32' autocomplete='off'",
    ));
    out.push_str(&field("wifi-password","Nouveau mot de passe Wi-Fi","password",String::new(),"maxlength='63' placeholder='Vide : conserver le mot de passe actuel' autocomplete='new-password'"));
    out.push_str("<label class='checkbox'><input id='open-network' type='checkbox'><span>Réseau ouvert (effacer le mot de passe)</span></label></div><p class='help'>Le nom est annoncé au serveur DHCP ; sa résolution dépend du routeur. Utilisez l’adresse IP du mineur pour vous connecter. mDNS et point d’accès de secours ne sont pas inclus. Vérifiez le SSID avant de redémarrer. La procédure de récupération USB figure dans le guide.</p>");
    let _ = write!(
        out,
        "<div class='data-row'><span>IPv4 actuelle</span><strong class='mono'>{}</strong></div>",
        text(info, "/ipv4")
    );
    out.push_str("<div class='form-actions'><p>Application après redémarrage.</p><button class='primary' type='submit'>Enregistrer le réseau</button></div></form></section>");
    out
}
fn logs(entries: &[Value]) -> String {
    let mut out = heading(
        "Journaux",
        "Événements de cette session : matériel, pool et réglages.",
    );
    out.push_str("<section class='panel'><div class='panel-heading'><h2>Événements récents</h2><a class='button' href='/api/system/logs' download='axeos-events.txt'>Télécharger</a></div><div class='log-console' role='log'>");
    if entries.is_empty() {
        out.push_str("<p class='muted'>Aucun événement enregistré.</p>");
    }
    for entry in entries {
        let _=write!(out,"<div class='log-line'><span class='log-time'>{} s</span><span class='log-level'>{}</span><span>{}</span></div>",num(entry,"/seconds",0),text(entry,"/level"),text(entry,"/message"));
    }
    out.push_str("</div><p class='help'>Les 32 derniers événements applicatifs sont conservés en RAM. Les logs de diagnostic complets restent accessibles via RTT.</p></section>");
    out
}
fn system(info: &Value) -> String {
    let mut out = heading(
        "Système",
        "Informations du firmware et maintenance du mineur.",
    );
    out.push_str("<div class='two-columns'><section class='panel'><h2>Votre Bitaxe</h2>");
    for (label, path) in [
        ("Carte", "/boardVersion"),
        ("ASIC", "/asicModel"),
        ("Firmware Rust", "/version"),
        ("Interface WebAssembly", "/axeOsVersion"),
    ] {
        out.push_str(&row(label, text(info, path)));
    }
    out.push_str(&row("Nom DHCP", text(info, "/settings/hostname")));
    out.push_str(&row("IPv4", text(info, "/ipv4")));
    out.push_str(&row(
        "Mémoire libre",
        format!("{} octets", num(info, "/freeHeap", 0)),
    ));
    out.push_str(&row(
        "Configuration persistante",
        if flag(info, "/storageAvailable") {
            "Partition axe_cfg détectée".into()
        } else {
            "Indisponible : installer la table de partitions".into()
        },
    ));
    out.push_str("</section><section class='panel'><h2>Maintenance</h2><p class='body-copy'>Le redémarrage applique les réglages enregistrés du réseau, du pool et de l’ASIC.</p><button class='primary' data-action='restart'>Redémarrer le mineur</button><hr><h3>Mise à jour par USB</h3><p class='body-copy'>L’interface web est intégrée au firmware. Flashez la nouvelle version avec espflash. La mise à jour OTA n’est pas prise en charge par ce port Rust.</p></section></div><p class='attribution'>Port Rust indépendant inspiré d’AxeOS / ESP-Miner. Les fonctionnalités disponibles sont celles reliées à ce firmware.</p>");
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_firmware_contract_renders_every_page_without_secrets() {
        let config = axeos_core::Config::defaults("wifi", "private-wifi", 525, 1150);
        let info = axeos_core::Info {
            device_model: "Gamma",
            asic_model: "BM1370",
            board_version: "601",
            version: "test",
            axe_os_version: "rust-test",
            uptime_seconds: 60,
            free_heap: 32000,
            ipv4: "192.168.1.20".try_into().unwrap(),
            wifi_connected: true,
            storage_available: true,
            restart_required: false,
            voltage_min: 1000,
            voltage_max: 1250,
            settings: config.public(),
            telemetry: axeos_core::Telemetry::default(),
        };
        let value = serde_json::to_value(info).unwrap();
        for page in [
            dashboard(&value, &[]),
            pool(&value),
            hardware(&value),
            network(&value),
            system(&value),
        ] {
            assert!(!page.contains("private-wifi"));
            assert!(!page.contains("NaN"));
        }
        assert!(hardware(&value).contains("value='1150'"));
        assert!(network(&value).contains("192.168.1.20"));
        assert!(system(&value).contains("rust-test"));
    }
    #[test]
    fn escapes_settings_before_rendering() {
        let v = json!({"settings":{"poolHost":"<script>alert(1)</script>\""}});
        let html = pool(&v);
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }
    #[test]
    fn missing_metrics_are_not_fabricated() {
        let v = json!({});
        assert_eq!(num(&v, "/power", 1), "—");
        assert!(chart(&[]).contains("premières mesures"));
    }
    #[test]
    fn graphs_use_only_supplied_samples() {
        let html = chart(&[json!({"hashRate":100.0}), json!({"hashRate":200.0})]);
        assert!(html.contains("hash-line"));
        assert!(html.contains("M48.0"));
    }
}
