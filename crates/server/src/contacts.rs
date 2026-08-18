use super::{
    ApiError, AppState, authenticated_account, validate_image_upload, validate_short_text,
};
use actix_web::{HttpRequest, HttpResponse, http::header, web};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::Datelike;
use db::entities::{
    Contact, ContactAddress, ContactDavSource, ContactDraft, ContactImportantDate, ContactMethod,
    ContactPhotoDraft,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

const MAX_CONTACT_IMPORT_BYTES: usize = 64 * 1024 * 1024;
const MAX_CONTACT_PHOTO_BYTES: usize = 10 * 1024 * 1024;

#[derive(Debug, Serialize)]
struct ContactsResponse {
    contacts: Vec<Contact>,
    dav_sources: Vec<ContactDavSource>,
    secret_storage_enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct ContactRequest {
    #[serde(default)]
    first_name: String,
    #[serde(default)]
    middle_name: String,
    #[serde(default)]
    last_name: String,
    #[serde(default)]
    nickname: String,
    #[serde(default)]
    pronouns: String,
    #[serde(default)]
    company: String,
    #[serde(default)]
    job_title: String,
    birthday: Option<String>,
    #[serde(default)]
    emails: Vec<ContactMethod>,
    #[serde(default)]
    phones: Vec<ContactMethod>,
    #[serde(default)]
    addresses: Vec<ContactAddress>,
    #[serde(default)]
    important_dates: Vec<ContactImportantDate>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    relationship_context: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    favorite: bool,
    #[serde(default)]
    archived: bool,
}

#[derive(Debug, Deserialize)]
struct ImportContactsRequest {
    format: String,
    payload: Value,
}

#[derive(Debug, Serialize)]
struct ImportContactsResponse {
    imported: usize,
    skipped: usize,
    total: usize,
}

#[derive(Debug, Deserialize)]
struct CreateDavSourceRequest {
    name: String,
    url: String,
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
}

#[derive(Debug, Serialize)]
struct DavSyncResponse {
    source: ContactDavSource,
    imported: usize,
}

pub fn configure(config: &mut web::ServiceConfig) {
    config
        .route("/contacts", web::get().to(list_contacts))
        .route("/contacts", web::post().to(create_contact))
        .route("/contacts/export", web::get().to(export_contacts))
        .service(
            web::resource("/contacts/{contact_id}/photo")
                .app_data(web::PayloadConfig::new(MAX_CONTACT_PHOTO_BYTES))
                .route(web::get().to(get_contact_photo))
                .route(web::put().to(update_contact_photo))
                .route(web::delete().to(delete_contact_photo)),
        )
        .service(
            web::resource("/contacts/import")
                .app_data(web::JsonConfig::default().limit(MAX_CONTACT_IMPORT_BYTES))
                .route(web::post().to(import_contacts)),
        )
        .route("/contacts/dav", web::post().to(create_dav_source))
        .route(
            "/contacts/dav/{source_id}/sync",
            web::post().to(sync_dav_source),
        )
        .route(
            "/contacts/dav/{source_id}",
            web::delete().to(delete_dav_source),
        )
        .route("/contacts/{contact_id}", web::put().to(update_contact))
        .route("/contacts/{contact_id}", web::delete().to(delete_contact));
}

async fn get_contact_photo(
    state: web::Data<AppState>,
    request: HttpRequest,
    contact_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let photo = db::contact_queries::find_contact_photo(&state.pool, &account.id, &contact_id)
        .await?
        .ok_or(ApiError::NotFound("contact photo not found"))?;

    Ok(HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, photo.mime_type))
        .insert_header((header::CACHE_CONTROL, "private, no-cache"))
        .insert_header((header::ETAG, format!("\"{}\"", photo.updated_at)))
        .insert_header(("Cross-Origin-Resource-Policy", "same-origin"))
        .insert_header(("X-Content-Type-Options", "nosniff"))
        .body(photo.image_data))
}

async fn update_contact_photo(
    state: web::Data<AppState>,
    request: HttpRequest,
    contact_id: web::Path<String>,
    image_data: web::Bytes,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if image_data.is_empty() || image_data.len() > MAX_CONTACT_PHOTO_BYTES {
        return Err(ApiError::BadRequest(
            "contact photo must be between 1 byte and 10 MB",
        ));
    }
    db::contact_queries::get_contact(&state.pool, &account.id, &contact_id)
        .await?
        .ok_or(ApiError::NotFound("contact not found"))?;
    let mime_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .ok_or(ApiError::BadRequest("contact photo type is required"))?;
    validate_image_upload(mime_type, &image_data, "contact photo")?;
    db::contact_queries::upsert_contact_photo(
        &state.pool,
        &account.id,
        &contact_id,
        mime_type,
        &image_data,
    )
    .await?;
    Ok(HttpResponse::NoContent().finish())
}

async fn delete_contact_photo(
    state: web::Data<AppState>,
    request: HttpRequest,
    contact_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    db::contact_queries::get_contact(&state.pool, &account.id, &contact_id)
        .await?
        .ok_or(ApiError::NotFound("contact not found"))?;
    db::contact_queries::delete_contact_photo(&state.pool, &account.id, &contact_id).await?;
    Ok(HttpResponse::NoContent().finish())
}

async fn list_contacts(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<web::Json<ContactsResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let (contacts, dav_sources) = tokio::try_join!(
        db::contact_queries::list_contacts(&state.pool, &account.id),
        db::contact_queries::list_dav_sources(&state.pool, &account.id),
    )?;
    Ok(web::Json(ContactsResponse {
        contacts,
        dav_sources,
        secret_storage_enabled: state.widget_integrations.secrets_enabled(),
    }))
}

async fn create_contact(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<ContactRequest>,
) -> Result<(web::Json<Contact>, actix_web::http::StatusCode), ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let draft = validated_contact(&payload, "manual", None, None)?;
    let contact = db::contact_queries::create_contact(&state.pool, &account.id, &draft).await?;
    Ok((web::Json(contact), actix_web::http::StatusCode::CREATED))
}

async fn update_contact(
    state: web::Data<AppState>,
    request: HttpRequest,
    contact_id: web::Path<String>,
    payload: web::Json<ContactRequest>,
) -> Result<web::Json<Contact>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let existing = db::contact_queries::get_contact(&state.pool, &account.id, &contact_id)
        .await?
        .ok_or(ApiError::NotFound("contact not found"))?;
    let draft = validated_contact(
        &payload,
        &existing.source_kind,
        existing.source_reference,
        existing.dav_source_id,
    )?;
    let contact =
        db::contact_queries::update_contact(&state.pool, &account.id, &contact_id, &draft)
            .await?
            .ok_or(ApiError::NotFound("contact not found"))?;
    Ok(web::Json(contact))
}

async fn delete_contact(
    state: web::Data<AppState>,
    request: HttpRequest,
    contact_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if db::contact_queries::delete_contact(&state.pool, &account.id, &contact_id).await? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("contact not found"))
    }
}

async fn export_contacts(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let contacts = db::contact_queries::list_contacts(&state.pool, &account.id).await?;
    let mut exported_contacts = Vec::with_capacity(contacts.len());
    for contact in contacts {
        let contact_id = contact.id.clone();
        let mut value = serde_json::to_value(contact)
            .map_err(|_| ApiError::Internal("contact export could not be encoded"))?;
        if let Some(photo) =
            db::contact_queries::find_contact_photo(&state.pool, &account.id, &contact_id).await?
            && let Some(object) = value.as_object_mut()
        {
            object.insert(
                "photo".to_owned(),
                json!({
                    "mime_type": photo.mime_type,
                    "data_base64": STANDARD.encode(photo.image_data),
                }),
            );
        }
        exported_contacts.push(value);
    }
    let body = serde_json::to_vec_pretty(&json!({
        "format": "pandan-contacts",
        "version": 2,
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "contacts": exported_contacts,
    }))
    .map_err(|_| ApiError::Internal("contact export could not be encoded"))?;
    Ok(HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, "application/json; charset=utf-8"))
        .insert_header((
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"pandan-contacts.json\"",
        ))
        .body(body))
}

async fn import_contacts(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<ImportContactsRequest>,
) -> Result<web::Json<ImportContactsResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if !matches!(payload.format.as_str(), "monica-json" | "pandan-json") {
        return Err(ApiError::BadRequest("unsupported contact import format"));
    }
    let objects = contact_objects(&payload.payload);
    let monica_field_types = monica_contact_field_types(&payload.payload);
    if objects.len() > 10_000 {
        return Err(ApiError::BadRequest(
            "contact import contains more than 10000 records",
        ));
    }
    let total = objects.len();
    let mut imported = 0;
    let mut skipped = 0;
    for object in objects {
        let draft = if payload.format == "pandan-json" {
            parse_pandan_contact(object)
        } else {
            parse_monica_contact(object, &monica_field_types)
        };
        let Some(draft) = draft.and_then(validated_imported_contact) else {
            skipped += 1;
            continue;
        };
        db::contact_queries::upsert_imported_contact(&state.pool, &account.id, &draft).await?;
        imported += 1;
    }
    Ok(web::Json(ImportContactsResponse {
        imported,
        skipped,
        total,
    }))
}

async fn create_dav_source(
    state: web::Data<AppState>,
    request: HttpRequest,
    payload: web::Json<CreateDavSourceRequest>,
) -> Result<(web::Json<ContactDavSource>, actix_web::http::StatusCode), ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let name = validate_short_text(&payload.name, "DAV source name is required", 80)?;
    let url = validate_short_text(&payload.url, "CardDAV address-book URL is required", 2_048)?;
    if payload.username.chars().count() > 320 || payload.password.chars().count() > 4_096 {
        return Err(ApiError::BadRequest("DAV credentials are too long"));
    }
    state
        .widget_integrations
        .validate_public_https_source(url)
        .await
        .map_err(ApiError::Integration)?;
    let encrypted = (!payload.password.is_empty())
        .then(|| state.widget_integrations.encrypt_secret(&payload.password))
        .transpose()
        .map_err(ApiError::Integration)?;
    let source = db::contact_queries::create_dav_source(
        &state.pool,
        &account.id,
        name,
        url,
        payload.username.trim(),
        encrypted.as_deref(),
    )
    .await?;
    Ok((web::Json(source), actix_web::http::StatusCode::CREATED))
}

async fn sync_dav_source(
    state: web::Data<AppState>,
    request: HttpRequest,
    source_id: web::Path<String>,
) -> Result<web::Json<DavSyncResponse>, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    let source = db::contact_queries::get_dav_source(&state.pool, &account.id, &source_id)
        .await?
        .ok_or(ApiError::NotFound("DAV source not found"))?;
    let encrypted_password =
        db::contact_queries::get_dav_password(&state.pool, &account.id, &source_id).await?;
    let remote_contacts = state
        .widget_integrations
        .fetch_carddav_contacts(
            &source.id,
            &source.url,
            &source.username,
            encrypted_password.as_deref(),
        )
        .await;
    let remote_contacts = match remote_contacts {
        Ok(contacts) => contacts,
        Err(error) => {
            db::contact_queries::set_dav_sync_status(
                &state.pool,
                &account.id,
                &source_id,
                Some(&error),
            )
            .await?;
            return Err(ApiError::Integration(error));
        }
    };
    let imported = remote_contacts.len();
    for contact in &remote_contacts {
        db::contact_queries::upsert_imported_contact(&state.pool, &account.id, contact).await?;
    }
    db::contact_queries::set_dav_sync_status(&state.pool, &account.id, &source_id, None).await?;
    let source = db::contact_queries::get_dav_source(&state.pool, &account.id, &source_id)
        .await?
        .ok_or(ApiError::NotFound("DAV source not found"))?;
    Ok(web::Json(DavSyncResponse { source, imported }))
}

async fn delete_dav_source(
    state: web::Data<AppState>,
    request: HttpRequest,
    source_id: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let account = authenticated_account(&state, &request).await?;
    if db::contact_queries::delete_dav_source(&state.pool, &account.id, &source_id).await? {
        Ok(HttpResponse::NoContent().finish())
    } else {
        Err(ApiError::NotFound("DAV source not found"))
    }
}

fn validated_contact(
    request: &ContactRequest,
    source_kind: &str,
    source_reference: Option<String>,
    dav_source_id: Option<String>,
) -> Result<ContactDraft, ApiError> {
    let first_name = clean_text(&request.first_name, 120)?;
    let middle_name = clean_text(&request.middle_name, 120)?;
    let last_name = clean_text(&request.last_name, 120)?;
    let nickname = clean_text(&request.nickname, 120)?;
    if first_name.is_empty() && last_name.is_empty() && nickname.is_empty() {
        return Err(ApiError::BadRequest("enter a contact name or nickname"));
    }
    let birthday = validated_birthday(request.birthday.as_deref())?;
    let emails = validate_methods(&request.emails, 30, 320)?;
    let phones = validate_methods(&request.phones, 30, 80)?;
    let addresses = validate_addresses(&request.addresses)?;
    let important_dates = validate_dates(&request.important_dates)?;
    let tags = validate_tags(&request.tags)?;
    Ok(ContactDraft {
        dav_source_id,
        source_kind: source_kind.to_owned(),
        source_reference,
        first_name,
        middle_name,
        last_name,
        nickname,
        pronouns: clean_text(&request.pronouns, 80)?,
        company: clean_text(&request.company, 160)?,
        job_title: clean_text(&request.job_title, 160)?,
        birthday,
        emails,
        phones,
        addresses,
        important_dates,
        tags,
        relationship_context: clean_text(&request.relationship_context, 4_000)?,
        notes: clean_text(&request.notes, 20_000)?,
        favorite: request.favorite,
        archived: request.archived,
        photo: None,
    })
}

fn validated_imported_contact(draft: ContactDraft) -> Option<ContactDraft> {
    let photo = draft.photo;
    let source_kind = draft.source_kind.clone();
    let source_reference = draft.source_reference.clone();
    let dav_source_id = draft.dav_source_id.clone();
    let request = ContactRequest {
        first_name: draft.first_name,
        middle_name: draft.middle_name,
        last_name: draft.last_name,
        nickname: draft.nickname,
        pronouns: draft.pronouns,
        company: draft.company,
        job_title: draft.job_title,
        birthday: draft.birthday,
        emails: draft.emails,
        phones: draft.phones,
        addresses: draft.addresses,
        important_dates: draft.important_dates,
        tags: draft.tags,
        relationship_context: draft.relationship_context,
        notes: draft.notes,
        favorite: draft.favorite,
        archived: draft.archived,
    };
    validated_contact(&request, &source_kind, source_reference, dav_source_id)
        .ok()
        .map(|mut validated| {
            validated.photo = photo;
            validated
        })
}

fn clean_text(value: &str, max_length: usize) -> Result<String, ApiError> {
    let value = value.trim();
    if value.chars().count() > max_length {
        return Err(ApiError::BadRequest("contact field is too long"));
    }
    Ok(value.to_owned())
}

fn validated_date(value: Option<&str>) -> Result<Option<String>, ApiError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| ApiError::BadRequest("contact dates must use YYYY-MM-DD"))?;
    Ok(Some(value.to_owned()))
}

fn validated_birthday(value: Option<&str>) -> Result<Option<String>, ApiError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_err()
        && birthday_month_day(value).is_none()
    {
        return Err(ApiError::BadRequest(
            "birthdays must use YYYY-MM-DD or --MM-DD when the year is unknown",
        ));
    }
    Ok(Some(value.to_owned()))
}

pub(crate) fn birthday_month_day(value: &str) -> Option<(u32, u32)> {
    let date = if let Some(month_day) = value.strip_prefix("--") {
        if month_day.len() != 5 || month_day.as_bytes().get(2) != Some(&b'-') {
            return None;
        }
        chrono::NaiveDate::parse_from_str(&format!("2000-{month_day}"), "%Y-%m-%d").ok()?
    } else {
        chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()?
    };
    Some((date.month(), date.day()))
}

fn validate_methods(
    methods: &[ContactMethod],
    maximum: usize,
    value_maximum: usize,
) -> Result<Vec<ContactMethod>, ApiError> {
    if methods.len() > maximum {
        return Err(ApiError::BadRequest("contact has too many contact methods"));
    }
    methods
        .iter()
        .filter(|method| !method.value.trim().is_empty())
        .map(|method| {
            Ok(ContactMethod {
                label: clean_text(&method.label, 40)?,
                value: clean_text(&method.value, value_maximum)?,
            })
        })
        .collect()
}

fn validate_addresses(addresses: &[ContactAddress]) -> Result<Vec<ContactAddress>, ApiError> {
    if addresses.len() > 20 {
        return Err(ApiError::BadRequest("contact has too many addresses"));
    }
    addresses
        .iter()
        .map(|address| {
            Ok(ContactAddress {
                label: clean_text(&address.label, 40)?,
                street: clean_text(&address.street, 240)?,
                city: clean_text(&address.city, 120)?,
                region: clean_text(&address.region, 120)?,
                postal_code: clean_text(&address.postal_code, 40)?,
                country: clean_text(&address.country, 120)?,
            })
        })
        .collect()
}

fn validate_dates(dates: &[ContactImportantDate]) -> Result<Vec<ContactImportantDate>, ApiError> {
    if dates.len() > 40 {
        return Err(ApiError::BadRequest("contact has too many important dates"));
    }
    dates
        .iter()
        .map(|date| {
            Ok(ContactImportantDate {
                label: clean_text(&date.label, 80)?,
                date: validated_date(Some(&date.date))?
                    .ok_or(ApiError::BadRequest("important date is required"))?,
                recurring: date.recurring,
            })
        })
        .collect()
}

fn validate_tags(tags: &[String]) -> Result<Vec<String>, ApiError> {
    if tags.len() > 30 {
        return Err(ApiError::BadRequest("contact has too many tags"));
    }
    let mut seen = HashSet::new();
    let mut values = Vec::new();
    for tag in tags {
        let tag = clean_text(tag, 40)?;
        if !tag.is_empty() && seen.insert(tag.to_ascii_lowercase()) {
            values.push(tag);
        }
    }
    Ok(values)
}

fn contact_objects(payload: &Value) -> Vec<&Value> {
    if let Some(array) = payload.as_array() {
        return array.iter().filter(|value| value.is_object()).collect();
    }
    let mut arrays = Vec::new();
    collect_contact_arrays(payload, &mut arrays);
    arrays
        .into_iter()
        .flat_map(|array| array.iter())
        .filter(|value| value.is_object())
        .collect()
}

fn collect_contact_arrays<'a>(value: &'a Value, arrays: &mut Vec<&'a Vec<Value>>) {
    match value {
        Value::Object(object) => {
            if object
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind.eq_ignore_ascii_case("contact"))
                && let Some(values) = object.get("values").and_then(Value::as_array)
            {
                arrays.push(values);
                return;
            }
            for (key, child) in object {
                if key.eq_ignore_ascii_case("contacts") {
                    if let Some(array) = child.as_array() {
                        arrays.push(array);
                    }
                } else if matches!(key.as_str(), "account" | "data" | "vaults" | "vault") {
                    collect_contact_arrays(child, arrays);
                }
            }
        }
        Value::Array(values) => {
            for child in values {
                collect_contact_arrays(child, arrays);
            }
        }
        _ => {}
    }
}

fn parse_pandan_contact(value: &Value) -> Option<ContactDraft> {
    let contact: Contact = serde_json::from_value(value.clone()).ok()?;
    let source_reference = Some(
        contact
            .source_reference
            .unwrap_or_else(|| format!("pandan-{}", contact.id)),
    );
    Some(ContactDraft {
        dav_source_id: None,
        photo: parse_pandan_photo(value),
        source_kind: "monica".to_owned(),
        source_reference,
        first_name: contact.first_name,
        middle_name: contact.middle_name,
        last_name: contact.last_name,
        nickname: contact.nickname,
        pronouns: contact.pronouns,
        company: contact.company,
        job_title: contact.job_title,
        birthday: contact.birthday,
        emails: contact.emails,
        phones: contact.phones,
        addresses: contact.addresses,
        important_dates: contact.important_dates,
        tags: contact.tags,
        relationship_context: contact.relationship_context,
        notes: contact.notes,
        favorite: contact.favorite,
        archived: contact.archived,
    })
}
fn parse_pandan_photo(value: &Value) -> Option<ContactPhotoDraft> {
    let photo = value.get("photo")?.as_object()?;
    let mime_type = photo.get("mime_type")?.as_str()?;
    let encoded = photo.get("data_base64")?.as_str()?;
    decode_contact_photo(mime_type, encoded)
}

pub(crate) fn parse_vcard_photo(input: &str) -> Option<ContactPhotoDraft> {
    for line in unfold_contact_vcard(input) {
        let Some((property, raw_value)) = line.split_once(':') else {
            continue;
        };
        if !property
            .split(';')
            .next()
            .is_some_and(|name| name.eq_ignore_ascii_case("PHOTO"))
        {
            continue;
        }

        let normalized = raw_value.trim().replace("\\,", ",");
        if normalized.to_ascii_lowercase().starts_with("data:") {
            let (metadata, encoded) = normalized.split_once(',')?;
            if !metadata
                .split(';')
                .any(|part| part.eq_ignore_ascii_case("base64"))
            {
                continue;
            }
            let mime_type = metadata.get(5..)?.split(';').next()?;
            if let Some(photo) = decode_contact_photo(mime_type, encoded) {
                return Some(photo);
            }
            continue;
        }

        let mime_type = property
            .split(';')
            .skip(1)
            .find_map(|parameter| {
                let (name, value) = parameter.split_once('=')?;
                name.eq_ignore_ascii_case("TYPE").then_some(value)
            })
            .and_then(normalized_photo_mime)?;
        if let Some(photo) = decode_contact_photo(mime_type, &normalized) {
            return Some(photo);
        }
    }
    None
}

fn unfold_contact_vcard(input: &str) -> Vec<String> {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<String> = Vec::new();
    for line in normalized.lines() {
        if line.starts_with([' ', '\t']) {
            if let Some(previous) = lines.last_mut() {
                previous.push_str(line.trim_start());
            }
        } else {
            lines.push(line.to_owned());
        }
    }
    lines
}

fn normalized_photo_mime(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "jpeg" | "jpg" | "image/jpeg" => Some("image/jpeg"),
        "png" | "image/png" => Some("image/png"),
        "webp" | "image/webp" => Some("image/webp"),
        "avif" | "image/avif" => Some("image/avif"),
        _ => None,
    }
}

fn decode_contact_photo(mime_type: &str, encoded: &str) -> Option<ContactPhotoDraft> {
    let mime_type = normalized_photo_mime(mime_type)?;
    let compact = encoded
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    let image_data = STANDARD.decode(compact).ok()?;
    if image_data.is_empty()
        || image_data.len() > MAX_CONTACT_PHOTO_BYTES
        || !valid_contact_photo_signature(mime_type, &image_data)
    {
        return None;
    }
    Some(ContactPhotoDraft {
        mime_type: mime_type.to_owned(),
        image_data,
    })
}

fn valid_contact_photo_signature(mime_type: &str, bytes: &[u8]) -> bool {
    match mime_type {
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "image/png" => bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]),
        "image/webp" => bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP",
        "image/avif" => bytes.len() >= 12 && &bytes[4..8] == b"ftyp",
        _ => false,
    }
}

#[derive(Debug, Default)]
struct MonicaContactFieldType {
    label: String,
    kind: String,
}

fn monica_contact_field_types(payload: &Value) -> HashMap<String, MonicaContactFieldType> {
    payload
        .pointer("/account/instance/contact_field_types")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|value| {
            let object = value.as_object()?;
            let uuid = string_at(object, &["uuid"]);
            let properties = object.get("properties")?.as_object()?;
            (!uuid.is_empty()).then(|| {
                (
                    uuid,
                    MonicaContactFieldType {
                        label: string_at(properties, &["name"]),
                        kind: string_at(properties, &["type"]),
                    },
                )
            })
        })
        .collect()
}

fn parse_monica_contact(
    value: &Value,
    field_types: &HashMap<String, MonicaContactFieldType>,
) -> Option<ContactDraft> {
    let object = value.as_object()?;
    if let Some(properties) = object.get("properties").and_then(Value::as_object)
        && properties.contains_key("first_name")
    {
        return parse_monica_v4_contact(object, properties, field_types);
    }
    let first_name = string_at(object, &["first_name", "firstName"]);
    let middle_name = string_at(object, &["middle_name", "middleName"]);
    let last_name = string_at(object, &["last_name", "lastName"]);
    let nickname = string_at(object, &["nickname", "nick_name"]);
    if first_name.is_empty() && last_name.is_empty() && nickname.is_empty() {
        return None;
    }
    let source_reference = string_value(object.get("id"))
        .or_else(|| string_value(object.get("uuid")))
        .unwrap_or_else(|| format!("monica-{:016x}", stable_hash(value.to_string().as_bytes())));
    let mut emails = Vec::new();
    let mut phones = Vec::new();
    for information in array_at(object, &["contact_information", "contact_informations"]) {
        let Some(info) = information.as_object() else {
            continue;
        };
        let data = string_at(info, &["data", "value"]);
        if data.is_empty() {
            continue;
        }
        let kind = nested_string(info, &["kind", "type", "contact_information_type"]);
        let label = nested_string(info, &["label", "name", "type"]);
        let method = ContactMethod {
            label: if label.is_empty() {
                "other".to_owned()
            } else {
                label
            },
            value: data,
        };
        if kind.to_ascii_lowercase().contains("phone") {
            phones.push(method);
        } else if kind.to_ascii_lowercase().contains("mail") || method.value.contains('@') {
            emails.push(method);
        }
    }
    append_direct_methods(object.get("emails"), &mut emails);
    append_direct_methods(object.get("phones"), &mut phones);
    let mut addresses = Vec::new();
    for address in array_at(object, &["addresses"]) {
        let Some(address) = address.as_object() else {
            continue;
        };
        addresses.push(ContactAddress {
            label: fallback(string_at(address, &["type", "name"]), "other"),
            street: string_at(address, &["line_1", "street", "address"]),
            city: string_at(address, &["city"]),
            region: string_at(address, &["province", "state", "region"]),
            postal_code: string_at(address, &["postal_code", "zip"]),
            country: string_at(address, &["country"]),
        });
    }
    let mut important_dates = Vec::new();
    for date in array_at(object, &["important_dates", "dates"]) {
        let Some(date) = date.as_object() else {
            continue;
        };
        let value = string_at(date, &["date", "value"]);
        if value.len() < 10 {
            continue;
        }
        important_dates.push(ContactImportantDate {
            label: fallback(nested_string(date, &["name", "type"]), "Important date"),
            date: value[..10].to_owned(),
            recurring: date
                .get("recurring")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        });
    }
    let birthday = string_at(object, &["birthday", "birthdate"]);
    let birthday = (birthday.len() >= 10).then(|| birthday[..10].to_owned());
    let tags = array_at(object, &["labels", "tags"])
        .into_iter()
        .filter_map(|tag| {
            tag.as_str()
                .map(str::to_owned)
                .or_else(|| tag.as_object().map(|item| string_at(item, &["name"])))
        })
        .filter(|tag| !tag.is_empty())
        .collect();
    Some(ContactDraft {
        dav_source_id: None,
        source_kind: "monica".to_owned(),
        source_reference: Some(source_reference),
        first_name,
        middle_name,
        last_name,
        nickname,
        photo: parse_vcard_photo(&string_at(object, &["vcard"])),
        pronouns: nested_string(object, &["pronoun", "pronouns"]),
        company: nested_string(object, &["company", "organization"]),
        job_title: string_at(object, &["job_position", "job_title", "title"]),
        birthday,
        emails,
        phones,
        addresses,
        important_dates,
        tags,
        relationship_context: String::new(),
        notes: string_at(object, &["notes", "note"]),
        favorite: object
            .get("is_favorite")
            .or_else(|| object.get("favorite"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        archived: !object
            .get("listed")
            .and_then(Value::as_bool)
            .unwrap_or(true),
    })
}

fn parse_monica_v4_contact(
    object: &serde_json::Map<String, Value>,
    properties: &serde_json::Map<String, Value>,
    field_types: &HashMap<String, MonicaContactFieldType>,
) -> Option<ContactDraft> {
    let first_name = string_at(properties, &["first_name"]);
    let last_name = string_at(properties, &["last_name"]);
    if first_name.is_empty() && last_name.is_empty() {
        return None;
    }

    let source_reference = monica_v4_source_reference(object);
    let (emails, phones, mut note_sections) = monica_v4_contact_methods(object, field_types);

    let addresses = monica_group_values(object, "address")
        .into_iter()
        .filter_map(|address| {
            let properties = address.get("properties")?.as_object()?;
            Some(ContactAddress {
                label: fallback(string_at(properties, &["name"]), "other"),
                street: string_at(properties, &["street"]),
                city: string_at(properties, &["city"]),
                region: string_at(properties, &["province"]),
                postal_code: string_at(properties, &["postal_code"]),
                country: string_at(properties, &["country"]),
            })
        })
        .collect();

    let important_dates = monica_group_values(object, "reminder")
        .into_iter()
        .filter_map(|reminder| {
            let properties = reminder.get("properties")?.as_object()?;
            let date = date_prefix(&string_at(properties, &["initial_date"]))?;
            let frequency = string_at(properties, &["frequency_type"]);
            Some(ContactImportantDate {
                label: fallback(string_at(properties, &["title"]), "Reminder"),
                date,
                recurring: !matches!(frequency.as_str(), "" | "once" | "one_time"),
            })
        })
        .collect();

    for note in monica_group_values(object, "note") {
        let Some(properties) = note.get("properties").and_then(Value::as_object) else {
            continue;
        };
        let body = string_at(properties, &["body"]);
        if !body.is_empty() {
            note_sections.push(body);
        }
    }

    let birthday = properties
        .get("birthdate")
        .and_then(Value::as_object)
        .and_then(|birthdate| {
            let date = string_at(birthdate, &["date"]);
            if birthdate
                .get("is_year_unknown")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                date.get(5..10).and_then(|month_day| {
                    let birthday = format!("--{month_day}");
                    birthday_month_day(&birthday).map(|_| birthday)
                })
            } else {
                date_prefix(&date)
            }
        });

    let tags = monica_v4_tags(properties);

    Some(ContactDraft {
        dav_source_id: None,
        source_kind: "monica".to_owned(),
        source_reference: Some(source_reference),
        first_name,
        middle_name: String::new(),
        photo: parse_vcard_photo(&string_at(properties, &["vcard"])),
        last_name,
        nickname: String::new(),
        pronouns: String::new(),
        company: string_at(properties, &["company"]),
        job_title: string_at(properties, &["job"]),
        birthday,
        emails,
        phones,
        addresses,
        important_dates,
        tags,
        relationship_context: string_at(properties, &["description"]),
        notes: note_sections.join("\n\n"),
        favorite: properties
            .get("is_starred")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        archived: !properties
            .get("is_active")
            .and_then(Value::as_bool)
            .unwrap_or(true),
    })
}

fn monica_v4_source_reference(object: &serde_json::Map<String, Value>) -> String {
    let source_reference = string_at(object, &["uuid"]);
    if source_reference.is_empty() {
        format!(
            "monica-{:016x}",
            stable_hash(Value::Object(object.clone()).to_string().as_bytes())
        )
    } else {
        source_reference
    }
}

fn monica_v4_tags(properties: &serde_json::Map<String, Value>) -> Vec<String> {
    let mut tags = array_at(properties, &["tags"])
        .into_iter()
        .filter_map(|tag| tag.as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    if properties
        .get("is_dead")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        tags.push("Deceased".to_owned());
    }
    tags
}

fn monica_v4_contact_methods(
    object: &serde_json::Map<String, Value>,
    field_types: &HashMap<String, MonicaContactFieldType>,
) -> (Vec<ContactMethod>, Vec<ContactMethod>, Vec<String>) {
    let mut emails = Vec::new();
    let mut phones = Vec::new();
    let mut note_sections = Vec::new();
    for field in monica_group_values(object, "contact_field") {
        let Some(field) = field.as_object() else {
            continue;
        };
        let Some(field_properties) = field.get("properties").and_then(Value::as_object) else {
            continue;
        };
        let data = string_at(field_properties, &["data"]);
        if data.is_empty() {
            continue;
        }
        let type_id = string_at(field_properties, &["type"]);
        let definition = field_types.get(&type_id);
        let label = definition
            .map(|definition| definition.label.as_str())
            .filter(|label| !label.is_empty())
            .unwrap_or("other")
            .to_owned();
        let kind = definition
            .map(|definition| definition.kind.to_ascii_lowercase())
            .unwrap_or_default();
        let label_lowercase = label.to_ascii_lowercase();
        let method = ContactMethod {
            label: label.clone(),
            value: data.clone(),
        };
        if kind == "phone" || label_lowercase.contains("phone") || label_lowercase == "whatsapp" {
            phones.push(method);
        } else if kind == "email" || label_lowercase.contains("mail") || data.contains('@') {
            emails.push(method);
        } else {
            note_sections.push(format!("{label}: {data}"));
        }
    }
    (emails, phones, note_sections)
}

fn monica_group_values<'a>(
    contact: &'a serde_json::Map<String, Value>,
    group_type: &str,
) -> Vec<&'a Value> {
    contact
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|group| {
            group
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind == group_type)
        })
        .filter_map(|group| group.get("values").and_then(Value::as_array))
        .flat_map(|values| values.iter())
        .collect()
}

fn date_prefix(value: &str) -> Option<String> {
    let value = value.get(..10)?;
    chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .ok()
        .map(|_| value.to_owned())
}

fn append_direct_methods(value: Option<&Value>, output: &mut Vec<ContactMethod>) {
    let Some(value) = value else { return };
    if let Some(text) = value.as_str() {
        output.push(ContactMethod {
            label: "other".to_owned(),
            value: text.to_owned(),
        });
    } else if let Some(values) = value.as_array() {
        for value in values {
            if let Some(text) = value.as_str() {
                output.push(ContactMethod {
                    label: "other".to_owned(),
                    value: text.to_owned(),
                });
            } else if let Some(item) = value.as_object() {
                let data = string_at(item, &["value", "data", "address", "number"]);
                if !data.is_empty() {
                    output.push(ContactMethod {
                        label: fallback(string_at(item, &["label", "type"]), "other"),
                        value: data,
                    });
                }
            }
        }
    }
}

fn array_at<'a>(object: &'a serde_json::Map<String, Value>, keys: &[&str]) -> Vec<&'a Value> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(Value::as_array))
        .map_or_else(Vec::new, |values| values.iter().collect())
}

fn string_at(object: &serde_json::Map<String, Value>, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| string_value(object.get(*key)))
        .unwrap_or_default()
}

fn nested_string(object: &serde_json::Map<String, Value>, keys: &[&str]) -> String {
    for key in keys {
        let Some(value) = object.get(*key) else {
            continue;
        };
        if let Some(text) = string_value(Some(value)) {
            return text;
        }
        if let Some(child) = value.as_object() {
            let text = string_at(child, &["name", "label", "type"]);
            if !text.is_empty() {
                return text;
            }
        }
    }
    String::new()
}

fn string_value(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(value) => Some(value.trim().to_owned()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn fallback(value: String, fallback: &str) -> String {
    if value.is_empty() {
        fallback.to_owned()
    } else {
        value
    }
}

fn stable_hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contact_import_limit_accommodates_large_exports() {
        assert_eq!(MAX_CONTACT_IMPORT_BYTES, 64 * 1024 * 1024);
    }

    #[test]
    fn monica_contact_export_maps_identity_channels_and_dates() {
        let payload = json!({
            "contacts": [{
                "id": "contact-7",
                "first_name": "Mara",
                "last_name": "Rivera",
                "job_position": "Producer",
                "company": { "name": "Northstar Studio" },
                "contact_information": [
                    { "data": "mara@example.com", "kind": "email", "label": "work" },
                    { "data": "+1-555-0199", "kind": "phone", "label": "mobile" }
                ],
                "important_dates": [{ "date": "2021-06-12", "type": { "name": "Anniversary" } }],
                "labels": [{ "name": "film" }]
            }]
        });
        let objects = contact_objects(&payload);
        let contact =
            parse_monica_contact(objects[0], &HashMap::new()).expect("Monica contact parses");

        assert_eq!(contact.source_reference.as_deref(), Some("contact-7"));
        assert_eq!(contact.first_name, "Mara");
        assert_eq!(contact.company, "Northstar Studio");
        assert_eq!(contact.emails[0].value, "mara@example.com");
        assert_eq!(contact.phones[0].label, "mobile");
        assert_eq!(contact.important_dates[0].label, "Anniversary");
        assert_eq!(contact.tags, ["film"]);
    }

    #[test]
    fn monica_v4_export_maps_typed_contact_collections() {
        let payload = json!({
            "version": "1.0-preview.1",
            "app_version": "4.1.2",
            "account": {
                "instance": {
                    "contact_field_types": [
                        {
                            "uuid": "email-type",
                            "properties": { "name": "Email", "type": "email" }
                        },
                        {
                            "uuid": "phone-type",
                            "properties": { "name": "Phone", "type": "phone" }
                        },
                        {
                            "uuid": "social-type",
                            "properties": { "name": "Instagram" }
                        }
                    ]
                },
                "data": [{
                    "type": "contact",
                    "count": 1,
                    "values": [{
                        "uuid": "contact-42",
                        "properties": {
                            "first_name": "Mara",
                            "last_name": "Rivera",
                            "birthdate": {
                                "date": "1990-06-12T00:00:00.000000Z",
                                "is_year_unknown": false
                            },
                            "company": "Northstar Studio",
                            "job": "Producer",
                            "vcard": r#"BEGIN:VCARD
PHOTO;VALUE=URI:data:image/png;base64\,iVBORw0KGgo=
END:VCARD"#,
                            "description": "Met through film work",
                            "is_active": false,
                            "is_dead": false,
                            "is_starred": true,
                            "tags": ["film"]
                        },
                        "data": [
                            {
                                "type": "contact_field",
                                "values": [
                                    { "properties": { "data": "mara@example.com", "type": "email-type" } },
                                    { "properties": { "data": "+1-555-0199", "type": "phone-type" } },
                                    { "properties": { "data": "mara-film", "type": "social-type" } }
                                ]
                            },
                            {
                                "type": "address",
                                "values": [{
                                    "properties": {
                                        "name": "work",
                                        "street": "100 Studio Way",
                                        "city": "Toronto",
                                        "province": "Ontario",
                                        "postal_code": "M5V 1A1",
                                        "country": "Canada"
                                    }
                                }]
                            },
                            {
                                "type": "note",
                                "values": [{ "properties": { "body": "Prefers email." } }]
                            },
                            {
                                "type": "reminder",
                                "values": [{
                                    "properties": {
                                        "title": "Work anniversary",
                                        "initial_date": "2021-09-03T00:00:00.000000Z",
                                        "frequency_type": "year"
                                    }
                                }]
                            }
                        ]
                    }]
                }]
            }
        });
        let objects = contact_objects(&payload);
        let field_types = monica_contact_field_types(&payload);
        let contact =
            parse_monica_contact(objects[0], &field_types).expect("Monica 4.x contact parses");

        assert_eq!(objects.len(), 1);
        assert_eq!(contact.source_reference.as_deref(), Some("contact-42"));
        assert_eq!(contact.first_name, "Mara");
        assert_eq!(contact.birthday.as_deref(), Some("1990-06-12"));
        assert_eq!(contact.company, "Northstar Studio");
        assert_eq!(contact.job_title, "Producer");
        assert_eq!(contact.emails[0].value, "mara@example.com");
        assert_eq!(contact.phones[0].value, "+1-555-0199");
        assert_eq!(contact.addresses[0].label, "work");
        assert_eq!(contact.important_dates[0].label, "Work anniversary");
        assert!(contact.notes.contains("Instagram: mara-film"));
        assert!(contact.notes.contains("Prefers email."));
        let photo = contact.photo.expect("embedded Monica photo parses");
        assert_eq!(photo.mime_type, "image/png");
        assert_eq!(
            photo.image_data,
            [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]
        );
        assert_eq!(contact.relationship_context, "Met through film work");
        assert!(contact.favorite);
        assert!(contact.archived);
    }

    #[test]
    fn monica_v4_unknown_birth_year_is_preserved_as_month_and_day() {
        let value = json!({
            "uuid": "contact-43",
            "properties": {
                "first_name": "Sam",
                "last_name": "Lee",
                "birthdate": {
                    "date": "2000-04-23T00:00:00.000000Z",
                    "is_year_unknown": true
                }
            },
            "data": []
        });
        let contact = parse_monica_contact(&value, &HashMap::new())
            .expect("Monica contact with partial birthday parses");

        assert_eq!(contact.birthday.as_deref(), Some("--04-23"));
        assert!(!contact.notes.contains("Birthday (year unknown)"));
    }

    #[test]
    fn contact_arrays_are_found_inside_vault_exports() {
        let payload = json!({
            "data": { "vaults": [{ "contacts": [{ "first_name": "Ada" }] }] }
        });
        assert_eq!(contact_objects(&payload).len(), 1);
    }
}
