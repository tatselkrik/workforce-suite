//! Private own photo; only company name/logo may survive sign-out in memory.
use super::*;
use axum::body::Bytes;
use axum::extract::Query;
use base64::{Engine, engine::general_purpose::STANDARD};
use image::ImageDecoder;
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read};

const INPUT_LIMIT: usize = 5 * 1024 * 1024;
const STORED_LIMIT: usize = 2 * 1024 * 1024;

#[derive(Clone, Default)]
pub(super) struct Branding {
    company: Option<Uuid>,
    name: Option<String>,
    logo: Option<Vec<u8>>,
    version: Option<Uuid>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Bootstrap {
    profile_digest: String,
    company_id: Uuid,
    company_name: String,
    logo_png_base64: Option<String>,
}
pub(super) fn load_bootstrap(path: &std::path::Path, config: &Config) -> Result<Branding> {
    if !path.exists() {
        return Ok(Branding::default());
    }
    let mut bytes = vec![];
    std::fs::File::open(path)?
        .take(3 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 3 * 1024 * 1024,
        "Invalid company-branding profile."
    );
    let b: Bootstrap =
        serde_json::from_slice(&bytes).context("Invalid company-branding profile.")?;
    let mut hash = Sha256::new();
    hash.update(config.url.trim().trim_end_matches('/'));
    hash.update([0]);
    hash.update(config.key.trim());
    ensure!(
        b.profile_digest == format!("{:x}", hash.finalize()) && !b.company_id.is_nil(),
        "Company branding belongs to another deployment."
    );
    ensure!(
        valid_name(&b.company_name),
        "Invalid company-branding name."
    );
    let logo = b
        .logo_png_base64
        .map(|v| {
            let raw = STANDARD.decode(v)?;
            ensure!(raw.len() <= STORED_LIMIT, "Company logo is too large.");
            normalize(&raw, false).map_err(|_| anyhow::anyhow!("Invalid company logo."))
        })
        .transpose()?;
    Ok(Branding {
        company: Some(b.company_id),
        name: Some(b.company_name.trim().into()),
        logo,
        version: None,
    })
}
fn valid_name(name: &str) -> bool {
    !name.trim().is_empty() && name.chars().count() <= 100 && !name.chars().any(char::is_control)
}
pub(super) fn routes() -> Router<Portal> {
    Router::new()
        .route("/pictures.js", get(script))
        .route("/api/branding", get(public_branding))
        .route("/api/branding/logo", get(public_logo))
        .route("/api/pictures", get(context))
        .route("/api/company-logo", get(company_logo))
        .route(
            "/api/photo",
            get(photo)
                .post(upload)
                .layer(DefaultBodyLimit::max(INPUT_LIMIT)),
        )
        .route("/api/photo/remove", post(remove))
}
async fn script() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../web/pictures.js"),
    )
}
fn image_reply(bytes: Vec<u8>) -> Response {
    ([(header::CONTENT_TYPE, "image/png")], bytes).into_response()
}
fn no_photo() -> Error {
    Error(StatusCode::NOT_FOUND, "No uploaded image is available.")
}
async fn public_branding(State(p): State<Portal>) -> Json<Value> {
    let b = p.branding.lock().await;
    Json(json!({"company_name":b.name,"has_logo":b.logo.is_some()}))
}
async fn public_logo(State(p): State<Portal>) -> Reply<Response> {
    Ok(image_reply(
        p.branding.lock().await.logo.clone().ok_or_else(no_photo)?,
    ))
}
async fn decode(p: &Portal, raw: Vec<u8>, avatar: bool) -> Reply<Vec<u8>> {
    let _slot = p.image_slots.acquire().await.map_err(|_| unavailable())?;
    tokio::task::spawn_blocking(move || normalize(&raw, avatar))
        .await
        .map_err(|_| unavailable())?
}
fn normalize(raw: &[u8], avatar: bool) -> Reply<Vec<u8>> {
    if raw.is_empty() || raw.len() > INPUT_LIMIT {
        return Err(invalid("Choose a PNG or JPEG image up to5MiB."));
    }
    let mut reader = image::ImageReader::new(Cursor::new(raw))
        .with_guessed_format()
        .map_err(|_| invalid("Choose a valid PNG or JPEG image."))?;
    if !matches!(
        reader.format(),
        Some(image::ImageFormat::Png | image::ImageFormat::Jpeg)
    ) {
        return Err(invalid("Choose a PNG or JPEG image."));
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(80 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader
        .into_decoder()
        .map_err(|_| invalid("Use a valid image up to4096 by4096pixels."))?;
    let orientation = decoder
        .orientation()
        .map_err(|_| invalid("Could not read the image orientation."))?;
    let mut decoded = image::DynamicImage::from_decoder(decoder)
        .map_err(|_| invalid("Could not decode that image."))?;
    decoded.apply_orientation(orientation);
    let output = if avatar {
        let side = decoded.width().min(decoded.height());
        if side == 0 {
            return Err(invalid("Choose a nonempty image."));
        }
        decoded
            .crop_imm(
                (decoded.width() - side) / 2,
                (decoded.height() - side) / 2,
                side,
                side,
            )
            .resize_exact(256, 256, image::imageops::FilterType::Lanczos3)
    } else {
        decoded.resize(512, 512, image::imageops::FilterType::Lanczos3)
    };
    let mut out = Cursor::new(vec![]);
    output
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|_| invalid("Could not prepare that image."))?;
    let bytes = out.into_inner();
    if bytes.len() > STORED_LIMIT {
        return Err(invalid("The prepared image is too large."));
    }
    Ok(bytes)
}
async fn stored_image(p: &Portal, s: &Session, path: &str) -> Reply<Vec<u8>> {
    let mut r = p
        .client
        .get(format!("{}{path}", p.config.url))
        // Storage edges may still serve a deleted/replaced object briefly even
        // with no-cache. A fresh origin read is required for private avatars and
        // for the current company-logo revision; never reuse a stale signed URL.
        .query(&[("cacheNonce", Uuid::new_v4().to_string())])
        .header("apikey", &p.config.key)
        .header("Cache-Control", "no-cache")
        .bearer_auth(&s.access)
        .send()
        .await
        .map_err(|_| unavailable())?;
    if !r.status().is_success() {
        let status = r.status();
        let error: Value = r.json().await.unwrap_or(Value::Null);
        if status == StatusCode::NOT_FOUND
            || (error["message"] == "Object not found"
                && (error["error"] == "not_found"
                    || error["code"] == "NoSuchKey"
                    || error["code"] == "not_found"))
        {
            return Err(no_photo());
        }
        return Err(unavailable());
    }
    if r.content_length().is_some_and(|n| n > STORED_LIMIT as u64) {
        return Err(unavailable());
    }
    let mut bytes = vec![];
    while let Some(chunk) = r.chunk().await.map_err(|_| unavailable())? {
        if bytes.len() + chunk.len() > STORED_LIMIT {
            return Err(unavailable());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
async fn company_row(p: &Portal, s: &Session) -> Reply<Value> {
    let r = first(
        p.upstream(
            &format!(
                "/rest/v1/companies?select=id,name,logo_version&id=eq.{}",
                s.company
            ),
            Method::GET,
            Some(&s.access),
            None,
        )
        .await?,
    )?;
    if id(&r, "id")? != s.company || r["name"].as_str().is_none_or(|n| !valid_name(n)) {
        return Err(unavailable());
    }
    Ok(r)
}
async fn load_company(p: &Portal, s: &Session) -> Reply<Branding> {
    let row = company_row(p, s).await?;
    let version = if row["logo_version"].is_null() {
        None
    } else {
        Some(id(&row, "logo_version")?)
    };
    {
        let mut b = p.branding.lock().await;
        if b.company.is_some_and(|c| c != s.company) {
            return Err(unavailable());
        }
        if b.company == Some(s.company)
            && b.version == version
            && version.is_some()
            && b.logo.is_some()
        {
            b.name = row["name"].as_str().map(String::from);
            return Ok(b.clone());
        }
    }
    let logo = if version.is_some() {
        Some(
            decode(
                p,
                stored_image(
                    p,
                    s,
                    &format!(
                        "/storage/v1/object/authenticated/company-branding/{}/logo.png",
                        s.company
                    ),
                )
                .await?,
                false,
            )
            .await?,
        )
    } else {
        None
    };
    if company_row(p, s).await?["logo_version"] != row["logo_version"] {
        return Err(Error(
            StatusCode::CONFLICT,
            "The company logo changed. Refresh to load it.",
        ));
    }
    let out = Branding {
        company: Some(s.company),
        name: row["name"].as_str().map(String::from),
        logo,
        version,
    };
    let mut b = p.branding.lock().await;
    if b.company.is_some_and(|c| c != s.company) {
        return Err(unavailable());
    }
    *b = out.clone();
    Ok(out)
}
async fn context(
    State(p): State<Portal>,
    h: HeaderMap,
    Query(_): Query<Empty>,
) -> Reply<Json<Value>> {
    let e = p.session(&h, false).await?;
    let s = e.lock().await;
    let b = load_company(&p, &s).await?;
    Ok(Json(
        json!({"company_name":b.name,"has_logo":b.logo.is_some()}),
    ))
}
async fn company_logo(
    State(p): State<Portal>,
    h: HeaderMap,
    Query(_): Query<Empty>,
) -> Reply<Response> {
    let e = p.session(&h, false).await?;
    let s = e.lock().await;
    Ok(image_reply(
        load_company(&p, &s).await?.logo.ok_or_else(no_photo)?,
    ))
}
async fn photo(State(p): State<Portal>, h: HeaderMap, Query(_): Query<Empty>) -> Reply<Response> {
    let e = p.session(&h, false).await?;
    let s = e.lock().await;
    let raw = stored_image(
        &p,
        &s,
        &format!(
            "/storage/v1/object/authenticated/profile-photos/{}/avatar.png",
            s.user
        ),
    )
    .await?;
    Ok(image_reply(decode(&p, raw, true).await?))
}
async fn upload(
    State(p): State<Portal>,
    h: HeaderMap,
    Query(_): Query<Empty>,
    raw: Bytes,
) -> Reply<Json<Value>> {
    let e = p.session(&h, true).await?;
    let s = e.lock().await;
    let png = decode(&p, raw.to_vec(), true).await?;
    let r = p
        .client
        .post(format!(
            "{}/storage/v1/object/profile-photos/{}/avatar.png",
            p.config.url, s.user
        ))
        .header("apikey", &p.config.key)
        .bearer_auth(&s.access)
        .header("Content-Type", "image/png")
        .header("x-upsert", "true")
        .header("Cache-Control", "max-age=0")
        .body(png)
        .send()
        .await
        .map_err(|_| {
            Error(
                StatusCode::BAD_GATEWAY,
                "The photo may have been saved. Refresh your profile before retrying.",
            )
        })?;
    if !r.status().is_success() {
        return Err(unavailable());
    }
    Ok(Json(json!({"saved":true})))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
async fn remove(
    State(p): State<Portal>,
    h: HeaderMap,
    Query(_): Query<Empty>,
    Json(_): Json<Empty>,
) -> Reply<Json<Value>> {
    let e = p.session(&h, true).await?;
    let s = e.lock().await;
    p.upstream(
        "/storage/v1/object/profile-photos",
        Method::DELETE,
        Some(&s.access),
        Some(json!({"prefixes":[format!("{}/avatar.png",s.user)]})),
    )
    .await?;
    Ok(Json(json!({"saved":true})))
}
#[cfg(test)]
mod tests {
    use super::*;
    pub(super) fn png() -> Vec<u8> {
        let mut c = Cursor::new(vec![]);
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            640,
            320,
            image::Rgba([24, 134, 124, 120]),
        ))
        .write_to(&mut c, image::ImageFormat::Png)
        .unwrap();
        c.into_inner()
    }
    #[test]
    fn photos_are_square_and_logos_keep_transparency_and_proportions() {
        let raw = png();
        let a = image::load_from_memory(&normalize(&raw, true).unwrap()).unwrap();
        let b = image::load_from_memory(&normalize(&raw, false).unwrap()).unwrap();
        assert_eq!((a.width(), a.height()), (256, 256));
        assert_eq!((b.width(), b.height()), (512, 256));
        assert_eq!(b.to_rgba8().get_pixel(0, 0).0[3], 120);
        assert!(normalize(b"<svg></svg>", true).is_err());
        assert!(normalize(&vec![0; INPUT_LIMIT + 1], true).is_err());
        assert!(!valid_name("bad\ncompany"));
    }

    #[test]
    fn startup_branding_is_bound_to_the_company_connection_and_safe_fields() {
        let folder = std::env::temp_dir().join(format!("hris-portal-branding-{}", Uuid::new_v4()));
        std::fs::create_dir(&folder).unwrap();
        let path = folder.join("hris-branding.json");
        let mut config = Config::origin("http://127.0.0.1:3030", "127.0.0.1:3030").unwrap();
        config.url = "https://fixture.supabase.co".into();
        config.key = "sb_publishable_fictional".into();
        let mut digest = Sha256::new();
        digest.update(config.url.as_bytes());
        digest.update([0]);
        digest.update(config.key.as_bytes());
        let mut value = json!({"profile_digest":format!("{:x}",digest.finalize()),"company_id":Uuid::from_u128(20),"company_name":"Fictional Company","logo_png_base64":STANDARD.encode(png())});
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let brand = load_bootstrap(&path, &config).unwrap();
        assert_eq!(brand.name.as_deref(), Some("Fictional Company"));
        assert!(brand.logo.is_some());
        config.key = "sb_publishable_another".into();
        assert!(load_bootstrap(&path, &config).is_err());
        config.key = "sb_publishable_fictional".into();
        value["password"] = json!("forbidden");
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(load_bootstrap(&path, &config).is_err());
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(folder).unwrap();
    }
}
