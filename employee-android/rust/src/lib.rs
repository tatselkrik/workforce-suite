//! Embeds the existing own-record Rust service inside the Android process.
//! The listener is device-loopback only; no PC service, disk session or proxy.
use anyhow::{Result, anyhow, ensure};
use hris_employee_portal::{Config, Portal};
use std::{net::TcpListener, path::Path, sync::{Mutex, OnceLock, mpsc}, thread, time::Duration};
use tokio::sync::oneshot;

struct Running { origin: String, stop: oneshot::Sender<()>, thread: thread::JoinHandle<()> }
static SERVER: OnceLock<Mutex<Option<Running>>> = OnceLock::new();

pub fn start(directory: &Path) -> Result<String> {
    let mut guard = SERVER.get_or_init(|| Mutex::new(None)).lock().map_err(|_|anyhow!("Service unavailable"))?;
    if let Some(server) = guard.as_ref() { return Ok(server.origin.clone()); }
    let profile = directory.join("hris-connection.json");
    ensure!(profile.is_file(), "Company deployment is missing");
    // Retain the bound socket: never release/rebind an ephemeral port.
    let socket = TcpListener::bind("127.0.0.1:0")?;
    socket.set_nonblocking(true)?;
    let address = socket.local_addr()?;
    ensure!(address.ip().is_loopback(), "Loopback required");
    let origin = format!("http://{address}");
    let config = Config::load(&profile, &origin, &address.to_string())?;
    let (stop, stopped) = oneshot::channel();
    let (ready, rx) = mpsc::sync_channel(1);
    let task = thread::Builder::new().name("hris-own-records".into()).spawn(move || {
        let runtime = match tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build() {
            Ok(value) => value, Err(_) => { let _ = ready.send(false); return; }
        };
        runtime.block_on(async move {
            let portal = match Portal::new(config) { Ok(value) => value, Err(_) => { let _ = ready.send(false); return; } };
            let listener = match tokio::net::TcpListener::from_std(socket) { Ok(value) => value, Err(_) => { let _ = ready.send(false); return; } };
            let _ = ready.send(true);
            let _ = axum::serve(listener, portal.router()).with_graceful_shutdown(async { let _ = stopped.await; }).await;
            let _ = tokio::time::timeout(Duration::from_secs(5), portal.close()).await;
        });
    })?;
    if rx.recv_timeout(Duration::from_secs(20)) != Ok(true) { let _ = stop.send(()); let _ = task.join(); return Err(anyhow!("Employee service did not start")); }
    *guard = Some(Running { origin: origin.clone(), stop, thread: task });
    Ok(origin)
}
pub fn stop() {
    let next = SERVER.get_or_init(||Mutex::new(None)).lock().ok().and_then(|mut value|value.take());
    if let Some(server) = next { let _ = server.stop.send(()); let _ = server.thread.join(); }
}

#[cfg(target_os = "android")]
mod android {
    use jni::{EnvUnowned, objects::{JObject, JString}};
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_io_github_tatselkrik_hrisemployee_RustPortal_start<'caller>(mut raw: EnvUnowned<'caller>, _this: JObject<'caller>, directory: JString<'caller>) -> JString<'caller> {
        raw.with_env(|env| -> Result<_, jni::errors::Error> {
            let path = directory.to_string();
            let reply = match super::start(std::path::Path::new(&path)) {
                Ok(origin) => serde_json::json!({"origin":origin}),
                Err(_) => serde_json::json!({"error":"The employee app could not start. Reopen it or reinstall the company package."}),
            };
            JString::from_str(env, reply.to_string())
        }).resolve::<jni::errors::ThrowRuntimeExAndDefault>()
    }
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_io_github_tatselkrik_hrisemployee_RustPortal_stop<'caller>(mut raw: EnvUnowned<'caller>, _this: JObject<'caller>) {
        raw.with_env(|_| -> Result<(), jni::errors::Error> { super::stop(); Ok(()) }).resolve::<jni::errors::ThrowRuntimeExAndDefault>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    #[test]
    fn held_loopback_socket_serves_only_expected_host_and_closes_on_shutdown() {
        assert!(start(Path::new("missing-company-deployment")).is_err());
        let directory = std::env::temp_dir().join(format!("hris-android-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("hris-connection.json"), r#"{"url":"https://fictional-mobile.supabase.co","publishable_key":"sb_publishable_fictional_mobile_test"}"#).unwrap();
        let origin = start(&directory).unwrap();
        assert!(origin.starts_with("http://127.0.0.1:"));
        assert_eq!(start(&directory).unwrap(), origin);
        let authority = origin.strip_prefix("http://").unwrap();
        let request = |host: &str| {
            let mut stream = std::net::TcpStream::connect(authority).unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            write!(stream, "GET / HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n").unwrap();
            let mut body = String::new(); stream.read_to_string(&mut body).unwrap(); body
        };
        let success = request(authority);
        assert!(success.starts_with("HTTP/1.1 200"));
        assert!(success.contains("no-store"));
        assert!(success.contains("Sign in"));
        let rejected = request("foreign.example.invalid");
        assert!(rejected.starts_with("HTTP/1.1 403") || rejected.starts_with("HTTP/1.1 400"));
        stop();
        assert!(std::net::TcpStream::connect(authority).is_err());
        // Remove only this generated, exact fixture directory after its contents are checked.
        std::fs::remove_file(directory.join("hris-connection.json")).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
