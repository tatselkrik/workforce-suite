//! Attendance-only shell with restricted, memory-only station authorization.
pub mod camera;
use crate::backend::{CloudClient, ConnectionConfig};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::{DateTime, Utc};
use eframe::egui;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Deserialize)]
struct Grant {
    token: String,
    station_id: Uuid,
    name: String,
    expires_at: DateTime<Utc>,
}
struct Punch {
    id: Uuid,
    code: String,
    pin: String,
    photo: Vec<u8>,
    operation: &'static str,
}
enum Event {
    Unlocked(Result<Grant, String>),
    Saved(Result<Value, (String, bool)>),
}
struct App {
    config: Option<ConnectionConfig>,
    station: Option<Uuid>,
    email: String,
    password: String,
    code: String,
    pin: String,
    grant: Option<Arc<Grant>>,
    pending: Option<Arc<Punch>>,
    camera: Option<camera::Camera>,
    frame: Option<camera::Frame>,
    texture: Option<egui::TextureHandle>,
    message: String,
    busy: bool,
    sender: mpsc::Sender<Event>,
    events: mpsc::Receiver<Event>,
    last_input: Instant,
}

pub fn run(station: Option<Uuid>) -> crate::desktop::AppResult {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id(crate::APP_ID)
            .with_inner_size([920.0, 820.0])
            .with_min_inner_size([680.0, 720.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Workforce HR · Attendance station",
        options,
        Box::new(move |cc| {
            crate::ui::configure_theme(&cc.egui_ctx);
            let (sender, events) = mpsc::channel();
            let config = ConnectionConfig::load()
                .ok()
                .filter(|c| c.validate().is_ok());
            Ok(Box::new(App {
                config,
                station,
                email: String::new(),
                password: String::new(),
                code: String::new(),
                pin: String::new(),
                grant: None,
                pending: None,
                camera: None,
                frame: None,
                texture: None,
                message: String::new(),
                busy: false,
                sender,
                events,
                last_input: Instant::now(),
            }))
        }),
    )
}
fn unlock(
    config: ConnectionConfig,
    station: Uuid,
    email: String,
    password: String,
) -> Result<Grant> {
    let (mut client, data) = CloudClient::connect(config, email, password)?;
    ensure!(
        data.access
            .allows(crate::staff_roles::Capability::AccountAdministration),
        "An Administrator must unlock the station."
    );
    let grant: Grant = serde_json::from_value(
        client.station_operation(&json!({"operation":"unlock","id":station}))?,
    )?;
    ensure!(
        grant.station_id == station && grant.token.len() == 64 && grant.expires_at > Utc::now(),
        "Station authorization did not match."
    );
    client.sign_out().context("Could not close the temporary Administrator sign-in. Unlock again before using this station.")?;
    Ok(grant)
}
fn send_punch(
    config: &ConnectionConfig,
    grant: &Grant,
    punch: &Punch,
) -> Result<Value, (String, bool)> {
    let request = json!({"token":grant.token,"request_id":punch.id,"employee_code":punch.code,"pin":punch.pin,"photo":STANDARD.encode(&punch.photo),"operation":punch.operation});
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(45))
        .connect_timeout(Duration::from_secs(7))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| ("Attendance connection unavailable.".into(), false))?;
    let response = client
        .post(format!(
            "{}/functions/v1/attendance-kiosk",
            config.url.trim_end_matches('/')
        ))
        .header("apikey", config.publishable_key.trim())
        .json(&request)
        .send()
        .map_err(|_| {
            (
                "Delivery is uncertain. Retry the same capture.".into(),
                true,
            )
        })?;
    let status = response.status();
    let value: Value = response
        .json()
        .map_err(|_| ("Receipt is uncertain. Retry the same capture.".into(), true))?;
    if !status.is_success() {
        return Err((
            value["error"]
                .as_str()
                .filter(|s| s.len() <= 500)
                .unwrap_or("Attendance could not be confirmed.")
                .into(),
            status.is_server_error(),
        ));
    }
    if value["saved"] != true || value["request_id"] != punch.id.to_string() {
        return Err((
            "Receipt did not match. Retry the same capture.".into(),
            true,
        ));
    }
    Ok(value)
}
impl App {
    fn clear_person(&mut self, ctx: &egui::Context) {
        self.code.clear();
        self.pin.clear();
        self.last_input = Instant::now();
        for name in ["kiosk-code", "kiosk-pin", "kiosk-password"] {
            let id = egui::Id::unique(name);
            if let Some(mut s) = egui::text_edit::TextEditState::load(ctx, id) {
                s.clear_undoer();
                s.store(ctx, id);
            }
        }
    }
    fn post(&mut self, punch: Arc<Punch>, ctx: &egui::Context) {
        let Some(config) = self.config.clone() else {
            return;
        };
        let Some(grant) = self.grant.clone() else {
            return;
        };
        self.pending = Some(punch.clone());
        self.busy = true;
        let sender = self.sender.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = send_punch(&config, &grant, &punch);
            let _ = sender.send(Event::Saved(result));
            ctx.request_repaint();
        });
    }
}
impl Drop for App {
    fn drop(&mut self) {
        if let (Some(config), Some(grant)) = (&self.config, &self.grant) {
            if let Ok(client) = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(4))
                .redirect(reqwest::redirect::Policy::none())
                .build()
            {
                let _ = client
                    .post(format!(
                        "{}/functions/v1/attendance-kiosk",
                        config.url.trim_end_matches('/')
                    ))
                    .header("apikey", config.publishable_key.trim())
                    .json(&json!({"operation":"close","token":grant.token}))
                    .send();
            }
        }
    }
}
impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let owned_context = ui.ctx().clone();
        let ctx = &owned_context;
        while let Ok(event) = self.events.try_recv() {
            self.busy = false;
            match event {
                Event::Unlocked(Ok(grant)) => {
                    self.email.clear();
                    self.clear_person(ctx);
                    self.grant = Some(Arc::new(grant));
                    self.camera = Some(camera::Camera::start());
                    self.message = "Look at the camera before recording attendance.".into();
                }
                Event::Unlocked(Err(error)) => self.message = error,
                Event::Saved(Ok(receipt)) => {
                    self.pending = None;
                    self.message = format!(
                        "{} recorded · {}",
                        receipt["operation"]
                            .as_str()
                            .unwrap_or("Attendance")
                            .replace('_', " "),
                        receipt["recorded_at"]
                            .as_str()
                            .unwrap_or("Company server time")
                    );
                    self.clear_person(ctx);
                }
                Event::Saved(Err((error, uncertain))) => {
                    if error.starts_with("Station is locked") {
                        self.grant = None;
                        self.camera = None;
                        self.frame = None;
                        self.texture = None;
                    }
                    self.message = error;
                    if !uncertain {
                        self.pending = None;
                        self.clear_person(ctx);
                    }
                }
            }
        }
        if let Some(camera) = &self.camera {
            while let Ok(frame) = camera.frames.try_recv() {
                match frame {
                    Ok(frame) => {
                        let image = egui::ColorImage::from_rgb(
                            [frame.image.width() as usize, frame.image.height() as usize],
                            frame.image.as_raw(),
                        );
                        if let Some(t) = &mut self.texture {
                            t.set(image, egui::TextureOptions::LINEAR);
                        } else {
                            self.texture = Some(ctx.load_texture(
                                "attendance-camera",
                                image,
                                egui::TextureOptions::LINEAR,
                            ));
                        }
                        self.frame = Some(frame);
                    }
                    Err(error) => {
                        self.message = error;
                        self.frame = None;
                        self.texture = None;
                    }
                }
            }
        }
        if self
            .grant
            .as_ref()
            .is_some_and(|g| g.expires_at <= Utc::now())
            && !self.busy
            && self.pending.is_none()
        {
            self.grant = None;
            self.camera = None;
            self.frame = None;
            self.texture = None;
            self.clear_person(ctx);
            self.message =
                "Authorization expired. Ask an Administrator to unlock this station.".into();
        }
        if self.pending.is_none()
            && !self.busy
            && self.last_input.elapsed() > Duration::from_secs(45)
        {
            self.clear_person(ctx);
        }
        if ctx.input(|i| i.viewport().close_requested()) && (self.busy || self.pending.is_some()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.message = "Confirm the pending punch before closing this station.".into();
        }
        egui::CentralPanel::default().show(ui,|ui|{
   ui.add_space(12.0);ui.heading("Workforce HR · Attendance station");
   if self.config.is_none()||self.station.is_none(){ui.label("Open the enrolled station from Workforce HR → Attendance → Office stations.");return;}
   if self.grant.is_none(){
    ui.label("An Administrator unlocks this registered office PC for the current session.");ui.label("Administrator email");ui.add(egui::TextEdit::singleline(&mut self.email).desired_width(420.0));ui.label("Password");let p=ui.add(egui::TextEdit::singleline(&mut self.password).password(true).id(egui::Id::unique("kiosk-password")).desired_width(420.0));
    let enter=p.lost_focus()&&ui.input(|i|i.key_pressed(egui::Key::Enter));
    if ui.add_enabled(!self.busy,egui::Button::new("Unlock station")).clicked()||(!self.busy&&enter){self.busy=true;let config=self.config.clone().unwrap();let station=self.station.unwrap();let email=self.email.clone();let password=std::mem::take(&mut self.password);let sender=self.sender.clone();let ctx=ctx.clone();std::thread::spawn(move||{let result=unlock(config,station,email,password).map_err(|e|e.to_string());let _=sender.send(Event::Unlocked(result));ctx.request_repaint();});}
   }else{
    ui.label(self.grant.as_ref().unwrap().name.as_str());ui.label("Use your employee code and personal attendance PIN.");
    ui.add_enabled_ui(!self.busy&&self.pending.is_none(),|ui|{
     ui.label("Employee code");let code=ui.add(egui::TextEdit::singleline(&mut self.code).id(egui::Id::unique("kiosk-code")).char_limit(80).desired_width(420.0));ui.label("Personal PIN");let pin=ui.add(egui::TextEdit::singleline(&mut self.pin).password(true).id(egui::Id::unique("kiosk-pin")).char_limit(12).desired_width(420.0));if code.changed()||pin.changed(){self.last_input=Instant::now();}
     if let Some(t)=&self.texture{ui.add(egui::Image::from_texture(t).max_size(egui::vec2(560.0,360.0)));}else{ui.label("A fresh webcam preview is required. Contact HR for a reasoned exception when the camera is unavailable.");if ui.button("Retry camera").clicked(){self.camera=Some(camera::Camera::start());}}
     ui.label("One photo is saved with your punch for authorized HR review.");let ready=!self.code.trim().is_empty()&&(6..=12).contains(&self.pin.len())&&self.pin.bytes().all(|b|b.is_ascii_digit())&&self.frame.is_some();
     ui.horizontal_wrapped(|ui|{for(operation,label)in[("check_in","Check in"),("check_out","Check out")]{if ui.add_enabled(ready,egui::Button::new(label).min_size(egui::vec2(180.0,48.0))).clicked(){match self.frame.as_ref().unwrap().photo(){Ok(photo)=>{let punch=Arc::new(Punch{id:Uuid::new_v4(),code:self.code.trim().into(),pin:std::mem::take(&mut self.pin),photo,operation});self.post(punch,ctx);},Err(error)=>self.message=error.to_string()}}}});
    });if !self.busy{if let Some(pending)=self.pending.clone(){if ui.button("Retry the same capture").clicked(){self.post(pending,ctx);}}}
   }if self.busy{ui.spinner();}ui.add_space(12.0);ui.label(&self.message);
  });
        ctx.request_repaint_after(Duration::from_millis(120));
    }
}
