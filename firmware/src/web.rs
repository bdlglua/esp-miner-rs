//! Bounded HTTP server. Static gzip assets are streamed directly from flash to the socket.
use crate::{settings, state};
use axeos_core::{
    http::{self, Method, Request},
    official::{self, SettingsError},
    websocket,
};
use core::fmt::Write as _;
use embassy_net::{
    tcp::{Error, TcpSocket},
    Stack,
};
use embassy_time::{with_deadline, with_timeout, Duration, Instant, Timer};
use embedded_io_async::Write as _;
use heapless::String;
use serde::Serialize;

mod assets {
    include!("../web-assets/official/assets.rs");
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Message<'a> {
    message: &'a str,
    restart_required: bool,
}

#[embassy_executor::task(pool_size = 4)]
pub async fn web_task(stack: Stack<'static>) {
    let mut rx = [0; 4096];
    let mut tx = [0; 4096];
    let mut request = [0; 4096];
    let mut json = [0; 12288];
    loop {
        stack.wait_config_up().await;
        let mut socket = TcpSocket::new(stack, &mut rx, &mut tx);
        socket.set_timeout(Some(Duration::from_secs(10)));
        if socket.accept(80).await.is_err() {
            Timer::after_millis(100).await;
            continue;
        }
        let result = serve(&mut socket, &mut request, &mut json).await;
        let restart = matches!(result, Ok(true));
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
    let deadline = Instant::now() + Duration::from_secs(15);
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
        let n = match with_deadline(deadline, socket.read(&mut buf[used..])).await {
            Ok(result) => result?,
            Err(_) => return Ok(false),
        };
        if n == 0 {
            return Ok(false);
        }
        used += n;
    };
    if req.method != Method::Get || req.websocket_key.is_some() {
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
    if req.method == Method::Get {
        if let Some((mime, body)) = assets::asset(req.path) {
            send(socket, 200, mime, body, true).await?;
            return Ok(false);
        }
    }
    match (req.method, req.path) {
        (Method::Get, "/api/ws/live" | "/api/ws") => {
            stream(socket, &req, json).await?;
        }
        (Method::Get, "/api/system/asic") => {
            let data = official::AsicSettings::new(
                crate::board::ASIC_MODEL,
                crate::board::DEVICE_MODEL,
                crate::board::HASH_FREQ_MHZ as u16,
                (crate::board::VCORE * 1000.0) as u16,
                crate::board::VOLTAGE_MIN_MV,
                crate::board::VOLTAGE_MAX_MV,
            );
            send_json(socket, 200, &data, json).await?;
        }
        (Method::Get, "/api/system/info") => {
            send_json(socket, 200, &official::SystemInfo(&state::info()), json).await?
        }
        (Method::Get, "/api/rust/system/info") => {
            send_json(socket, 200, &state::info(), json).await?
        }
        (Method::Get, "/api/system/statistics") => match official::columns(req.query) {
            Ok(labels) => {
                let history = state::with(|s| s.history.clone());
                let data = official::Statistics {
                    history: &history,
                    labels,
                    now_ms: embassy_time::Instant::now().as_millis(),
                };
                send_json(socket, 200, &data, json).await?;
            }
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
        },
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
            let result = official::parse_settings(req.body).and_then(|patch| {
                state::with(|s| {
                    patch.apply(
                        &s.config,
                        crate::board::VOLTAGE_MIN_MV,
                        crate::board::VOLTAGE_MAX_MV,
                        s.telemetry.fault,
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
                Err(error) => {
                    let (code, message) = match error {
                        SettingsError::Invalid(m) => (400, m),
                        SettingsError::Unsupported(m) => (501, m),
                    };
                    send_json(
                        socket,
                        code,
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
        (
            _,
            "/api/system/wifi/scan"
            | "/api/system/scoreboard"
            | "/api/system/identify"
            | "/api/system/blockFound/dismiss"
            | "/api/system/boot"
            | "/api/system/firmware/checksum",
        )
        | (Method::Delete, _) => {
            send_json(
                socket,
                501,
                &Message {
                    message: "Fonction non implémentée dans le firmware Rust.",
                    restart_required: false,
                },
                json,
            )
            .await?;
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
#[derive(Serialize)]
struct LiveUpdate<'a> {
    event: &'static str,
    data: official::SystemInfo<'a>,
}

struct StreamLease;
impl Drop for StreamLease {
    fn drop(&mut self) {
        state::with(|s| s.streaming_clients -= 1);
    }
}

async fn frame(socket: &mut TcpSocket<'_>, opcode: u8, body: &[u8]) -> Result<(), Error> {
    let n = body.len();
    if n <= 125 {
        socket.write_all(&[0x80 | opcode, n as u8]).await?;
    } else {
        socket
            .write_all(&[0x80 | opcode, 126, (n >> 8) as u8, n as u8])
            .await?;
    }
    socket.write_all(body).await?;
    socket.flush().await
}

/// Two streaming clients and two ordinary HTTP clients can coexist.
/// Incoming control frames are bounded and parsed across fragmented TCP reads.
async fn stream(
    socket: &mut TcpSocket<'_>,
    req: &Request<'_>,
    json: &mut [u8],
) -> Result<(), Error> {
    let Some(accept) = req.websocket_key.and_then(websocket::accept) else {
        return send_json(
            socket,
            400,
            &Message {
                message: "WebSocket upgrade required.",
                restart_required: false,
            },
            json,
        )
        .await;
    };
    let reserved = state::with(|s| {
        if s.streaming_clients >= 2 {
            false
        } else {
            s.streaming_clients += 1;
            true
        }
    });
    if !reserved {
        return send_json(
            socket,
            503,
            &Message {
                message: "Deux flux WebSocket sont déjà ouverts.",
                restart_required: false,
            },
            json,
        )
        .await;
    }
    let _lease = StreamLease;
    let mut header = String::<256>::new();
    let _=write!(header,"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",accept);
    socket.write_all(header.as_bytes()).await?;
    socket.flush().await?;
    let mut buffer = [0; 512];
    let mut used = 0;
    let mut log_sequence = 0;
    loop {
        if req.path == "/api/ws/live" {
            let snapshot = state::info();
            let update = LiveUpdate {
                event: "update",
                data: official::SystemInfo(&snapshot),
            };
            if let Ok(n) = serde_json_core::to_slice(&update, json) {
                frame(socket, 1, &json[..n]).await?;
            }
        } else {
            let (sequence, logs) = state::with(|s| (s.log_sequence, s.logs.clone()));
            let count = (sequence.saturating_sub(log_sequence) as usize).min(logs.len());
            for entry in &logs[logs.len() - count..] {
                let mut line = String::<160>::new();
                let _ = writeln!(
                    line,
                    "[{}s] {} {}",
                    entry.seconds, entry.level, entry.message
                );
                frame(socket, 1, line.as_bytes()).await?;
            }
            log_sequence = sequence;
            frame(socket, 9, b"").await?;
        }
        match with_timeout(Duration::from_secs(3), socket.read(&mut buffer[used..])).await {
            Err(_) => {
                continue;
            }
            Ok(Err(error)) => return Err(error),
            Ok(Ok(0)) => return Ok(()),
            Ok(Ok(n)) => used += n,
        }
        loop {
            let consumed = match websocket::client_frame(&mut buffer[..used]) {
                Ok(Some(incoming)) => {
                    match incoming.opcode {
                        8 => {
                            frame(socket, 8, &1000u16.to_be_bytes()).await?;
                            return Ok(());
                        }
                        9 => frame(socket, 10, incoming.payload).await?,
                        _ => {}
                    }
                    incoming.consumed
                }
                Ok(None) if used < 512 => break,
                _ => {
                    frame(socket, 8, &1002u16.to_be_bytes()).await?;
                    return Ok(());
                }
            };
            buffer.copy_within(consumed..used, 0);
            used -= consumed;
        }
    }
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
    let _=write!(header,"HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; font-src 'self' data:; connect-src 'self' ws: wss: https://api.github.com https://api.coinbase.com; img-src 'self' data:; object-src 'none'; base-uri 'self'; frame-ancestors 'none'\r\n",status,reason,kind,body.len());
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
