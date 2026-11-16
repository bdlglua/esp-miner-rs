//! Bounded HTTP server. Static gzip assets are streamed directly from flash to the socket.
use crate::{settings, state};
use axeos_core::{
    http::{self, Method, Request},
    Patch,
};
use core::fmt::Write as _;
use embassy_net::{
    tcp::{Error, TcpSocket},
    Stack,
};
use embassy_time::{with_timeout, Duration, Timer};
use embedded_io_async::Write as _;
use heapless::String;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Message<'a> {
    message: &'a str,
    restart_required: bool,
}

#[embassy_executor::task(pool_size = 2)]
pub async fn web_task(stack: Stack<'static>) {
    let mut rx = [0; 4096];
    let mut tx = [0; 4096];
    let mut request = [0; 4096];
    let mut json = [0; 8192];
    loop {
        stack.wait_config_up().await;
        let mut socket = TcpSocket::new(stack, &mut rx, &mut tx);
        socket.set_timeout(Some(Duration::from_secs(10)));
        if socket.accept(80).await.is_err() {
            Timer::after_millis(100).await;
            continue;
        }
        let result = with_timeout(
            Duration::from_secs(15),
            serve(&mut socket, &mut request, &mut json),
        )
        .await;
        let restart = matches!(result, Ok(Ok(true)));
        socket.close();
        let _ = with_timeout(Duration::from_secs(2), socket.flush()).await;
        // Give the peer a chance to acknowledge the final body before resetting the socket.
        Timer::after_millis(50).await;
        if restart {
            Timer::after_millis(300).await;
            esp_hal::system::software_reset();
        }
        socket.abort();
    }
}
async fn serve(socket: &mut TcpSocket<'_>, buf: &mut [u8], json: &mut [u8]) -> Result<bool, Error> {
    let mut used = 0;
    let req = loop {
        match http::parse(&buf[..used]) {
            Ok(Some(req)) => break req,
            Err(err) => {
                let code = match err {
                    http::Error::TooLarge => 413,
                    http::Error::Forbidden => 403,
                    http::Error::UnsupportedMedia => 415,
                    _ => 400,
                };
                send_json(
                    socket,
                    code,
                    &Message {
                        message: "Requête HTTP invalide.",
                        restart_required: false,
                    },
                    json,
                )
                .await?;
                return Ok(false);
            }
            Ok(None) => {}
        }
        if used == buf.len() {
            send(
                socket,
                413,
                "text/plain; charset=utf-8",
                b"Requete trop longue",
                false,
            )
            .await?;
            return Ok(false);
        }
        let n = socket.read(&mut buf[used..]).await?;
        if n == 0 {
            return Ok(false);
        }
        used += n;
    };
    if req.method != Method::Get {
        let (mut ip, active_hostname, pending_hostname) = state::with(|s| {
            (
                s.ip.clone(),
                s.active.hostname.clone(),
                s.config.hostname.clone(),
            )
        });
        if let Some(endpoint) = socket.local_endpoint() {
            ip.clear();
            let _ = write!(ip, "{}", endpoint.addr);
        }
        if !http::trusted_host(req.host, &ip, &active_hostname)
            && !http::trusted_host(req.host, &ip, &pending_hostname)
        {
            send_json(
                socket,
                403,
                &Message {
                    message: "Hôte non autorisé.",
                    restart_required: false,
                },
                json,
            )
            .await?;
            return Ok(false);
        }
    }
    route(socket, req, json).await
}
async fn route(
    socket: &mut TcpSocket<'_>,
    req: Request<'_>,
    json: &mut [u8],
) -> Result<bool, Error> {
    match (req.method, req.path) {
        (Method::Get, "/") => {
            send(
                socket,
                200,
                "text/html; charset=utf-8",
                include_bytes!("../web-assets/index.html"),
                false,
            )
            .await?
        }
        (Method::Get, "/style.css") => {
            send(
                socket,
                200,
                "text/css; charset=utf-8",
                include_bytes!("../web-assets/style.css.gz"),
                true,
            )
            .await?
        }
        (Method::Get, "/boot.js") => {
            send(
                socket,
                200,
                "text/javascript; charset=utf-8",
                include_bytes!("../web-assets/boot.js"),
                false,
            )
            .await?
        }
        (Method::Get, "/axeos_web.js") => {
            send(
                socket,
                200,
                "text/javascript; charset=utf-8",
                include_bytes!("../web-assets/axeos_web.js.gz"),
                true,
            )
            .await?
        }
        (Method::Get, "/axeos_web_bg.wasm") => {
            send(
                socket,
                200,
                "application/wasm",
                include_bytes!("../web-assets/axeos_web_bg.wasm.gz"),
                true,
            )
            .await?
        }
        (Method::Get, "/api/system/info") => send_json(socket, 200, &state::info(), json).await?,
        (Method::Get, "/api/system/statistics") => {
            let data = state::with(|s| s.history.clone());
            send_json(socket, 200, &data, json).await?;
        }
        (Method::Get, "/api/system/events") => {
            let data = state::with(|s| s.logs.clone());
            send_json(socket, 200, &data, json).await?;
        }
        (Method::Get, "/api/system/logs") => {
            let data = state::with(|s| s.logs.clone());
            let mut text = String::<6144>::new();
            for entry in data {
                let _ = writeln!(
                    text,
                    "[{}s] {} {}",
                    entry.seconds, entry.level, entry.message
                );
            }
            send(
                socket,
                200,
                "text/plain; charset=utf-8",
                text.as_bytes(),
                false,
            )
            .await?;
        }
        (Method::Patch, "/api/system") => {
            let result = serde_json_core::from_slice::<Patch>(req.body)
                .ok()
                .filter(|(_, n)| *n == req.body.len())
                .ok_or("JSON invalide ou champ inconnu.")
                .and_then(|(patch, _)| {
                    state::with(|s| {
                        s.config.apply(
                            patch,
                            crate::board::VOLTAGE_MIN_MV,
                            crate::board::VOLTAGE_MAX_MV,
                        )
                    })
                });
            match result {
                Ok(config) => match settings::save(&config) {
                    Ok(()) => {
                        let restart = state::with(|s| {
                            s.config = config;
                            s.restart_required = s.config.hostname != s.active.hostname
                                || s.config.ssid != s.active.ssid
                                || s.config.wifi_password != s.active.wifi_password
                                || s.config.pool_host != s.active.pool_host
                                || s.config.pool_port != s.active.pool_port
                                || s.config.pool_user != s.active.pool_user
                                || s.config.pool_password != s.active.pool_password
                                || s.config.frequency != s.active.frequency
                                || s.config.core_voltage != s.active.core_voltage;
                            s.restart_required
                        });
                        state::log("info", format_args!("Réglages enregistrés en flash"));
                        send_json(
                            socket,
                            200,
                            &Message {
                                message: "Réglages enregistrés.",
                                restart_required: restart,
                            },
                            json,
                        )
                        .await?;
                    }
                    Err(message) => {
                        send_json(
                            socket,
                            503,
                            &Message {
                                message,
                                restart_required: false,
                            },
                            json,
                        )
                        .await?
                    }
                },
                Err(message) => {
                    send_json(
                        socket,
                        400,
                        &Message {
                            message,
                            restart_required: false,
                        },
                        json,
                    )
                    .await?
                }
            }
        }
        (Method::Post, "/api/system/restart") => {
            state::log("info", format_args!("Redémarrage demandé depuis AxeOS"));
            send_json(
                socket,
                200,
                &Message {
                    message: "Redémarrage en cours.",
                    restart_required: false,
                },
                json,
            )
            .await?;
            return Ok(true);
        }
        (Method::Post, "/api/system/pause") => {
            state::with(|s| {
                s.telemetry.mining_paused = true;
                s.telemetry.hash_rate = None;
            });
            state::log("info", format_args!("Pause du minage demandée"));
            send_json(
                socket,
                200,
                &Message {
                    message: "Pause demandée. La chaîne ASIC va être remise en reset.",
                    restart_required: false,
                },
                json,
            )
            .await?;
        }
        (Method::Post, "/api/system/resume") => {
            let fault = state::with(|s| s.telemetry.fault);
            if fault {
                send_json(
                    socket,
                    409,
                    &Message {
                        message: "Défaut matériel verrouillé : contrôler la carte puis redémarrer.",
                        restart_required: false,
                    },
                    json,
                )
                .await?;
            } else {
                state::with(|s| s.telemetry.mining_paused = false);
                state::log("info", format_args!("Reprise du minage demandée"));
                send_json(
                    socket,
                    200,
                    &Message {
                        message: "Reprise demandée.",
                        restart_required: false,
                    },
                    json,
                )
                .await?;
            }
        }
        (Method::Post, "/api/system/OTA" | "/api/system/OTAWWW") => {
            send_json(
                socket,
                501,
                &Message {
                    message: "Mise à jour USB uniquement sur ce port Rust.",
                    restart_required: false,
                },
                json,
            )
            .await?
        }
        _ => {
            send_json(
                socket,
                404,
                &Message {
                    message: "Ressource introuvable.",
                    restart_required: false,
                },
                json,
            )
            .await?
        }
    }
    Ok(false)
}
async fn send_json(
    socket: &mut TcpSocket<'_>,
    status: u16,
    value: &impl Serialize,
    buf: &mut [u8],
) -> Result<(), Error> {
    match serde_json_core::to_slice(value, buf) {
        Ok(n) => {
            send(
                socket,
                status,
                "application/json; charset=utf-8",
                &buf[..n],
                false,
            )
            .await
        }
        Err(_) => {
            send(
                socket,
                500,
                "application/json",
                br#"{"message":"Reponse trop longue"}"#,
                false,
            )
            .await
        }
    }
}
async fn send(
    socket: &mut TcpSocket<'_>,
    status: u16,
    kind: &str,
    body: &[u8],
    gzip: bool,
) -> Result<(), Error> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        413 => "Content Too Large",
        415 => "Unsupported Media Type",
        501 => "Not Implemented",
        503 => "Service Unavailable",
        _ => "Internal Server Error",
    };
    let mut header = String::<1024>::new();
    let _=write!(header,"HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; connect-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'\r\n",status,reason,kind,body.len());
    if gzip {
        let _ = header.push_str("Content-Encoding: gzip\r\nVary: Accept-Encoding\r\n");
    }
    let _ = header.push_str("\r\n");
    socket.write_all(header.as_bytes()).await?;
    for chunk in body.chunks(1460) {
        socket.write_all(chunk).await?;
    }
    socket.flush().await
}
#[embassy_executor::task]
pub async fn stats_task(stack: Stack<'static>) {
    loop {
        state::with(|s| {
            s.wifi_connected = stack.is_link_up();
            s.ip.clear();
            if let Some(config) = stack.config_v4() {
                let _ = write!(s.ip, "{}", config.address.address());
            }
        });
        state::sample();
        Timer::after_secs(5).await;
    }
}
