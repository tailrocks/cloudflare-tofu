use std::{collections::BTreeMap, env, fs};

use anyhow::{Context, Result, anyhow, bail};
use reqwest::blocking::Client;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::Deserialize;
use serde_json::{Map, Value, json};

#[derive(Debug, Deserialize)]
struct CloudflareEnvelope<T> {
    success: bool,
    result: T,
    #[serde(default)]
    errors: Vec<CloudflareError>,
    result_info: Option<ResultInfo>,
}

#[derive(Debug, Deserialize)]
struct CloudflareError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct ResultInfo {
    total_pages: u64,
}

#[derive(Debug, Deserialize)]
struct Zone {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct DnsRecord {
    id: String,
    name: String,
    #[serde(rename = "type")]
    record_type: String,
    ttl: u64,
    content: Option<String>,
    proxied: Option<bool>,
    priority: Option<u64>,
    comment: Option<String>,
    tags: Option<Vec<String>>,
    data: Option<Value>,
    settings: Option<Value>,
}

struct Cloudflare {
    client: Client,
}

impl Cloudflare {
    fn from_env() -> Result<Self> {
        let api_token = env::var("CLOUDFLARE_API_TOKEN").ok();
        let api_key = env::var("CLOUDFLARE_API_KEY").ok();
        let email = env::var("CLOUDFLARE_EMAIL").ok();

        let mut headers = HeaderMap::new();
        if let Some(token) = api_token {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {token}"))
                    .context("invalid CLOUDFLARE_API_TOKEN header value")?,
            );
        } else if let (Some(email), Some(api_key)) = (email, api_key) {
            headers.insert(
                "X-Auth-Email",
                HeaderValue::from_str(&email).context("invalid CLOUDFLARE_EMAIL header value")?,
            );
            headers.insert(
                "X-Auth-Key",
                HeaderValue::from_str(&api_key)
                    .context("invalid CLOUDFLARE_API_KEY header value")?,
            );
        } else {
            bail!("set CLOUDFLARE_API_TOKEN, or CLOUDFLARE_EMAIL plus CLOUDFLARE_API_KEY");
        }

        let client = Client::builder()
            .default_headers(headers)
            .build()
            .context("build Cloudflare HTTP client")?;

        Ok(Self { client })
    }

    fn get<T>(&self, path: &str, params: &[(&str, String)]) -> Result<CloudflareEnvelope<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        let url = format!("https://api.cloudflare.com/client/v4{path}");
        let response = self
            .client
            .get(url)
            .query(params)
            .send()
            .with_context(|| format!("GET {path}"))?;
        let status = response.status();
        let envelope: CloudflareEnvelope<T> = response
            .json()
            .with_context(|| format!("decode Cloudflare response for {path}"))?;

        if !status.is_success() || !envelope.success {
            let messages = envelope
                .errors
                .iter()
                .map(|error| error.message.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(anyhow!(
                "Cloudflare API failed for {path}: {}",
                if messages.is_empty() {
                    status.to_string()
                } else {
                    messages
                }
            ));
        }

        Ok(envelope)
    }

    fn list_all<T>(&self, path: &str, params: &[(&str, String)]) -> Result<Vec<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        let mut results = Vec::new();
        let mut page = 1_u64;

        loop {
            let mut page_params = params.to_vec();
            page_params.push(("page", page.to_string()));
            page_params.push(("per_page", "100".to_string()));

            let envelope: CloudflareEnvelope<Vec<T>> = self.get(path, &page_params)?;
            results.extend(envelope.result);

            let total_pages = envelope
                .result_info
                .map(|info| info.total_pages)
                .unwrap_or(page);
            if page >= total_pages {
                return Ok(results);
            }
            page += 1;
        }
    }
}

fn main() -> Result<()> {
    let zone_name =
        env::var("CLOUDFLARE_ZONE_NAME").unwrap_or_else(|_| "tailrocks.com".to_string());
    let cloudflare = Cloudflare::from_env()?;

    let zones: Vec<Zone> = cloudflare.list_all(
        "/zones",
        &[
            ("name", zone_name.clone()),
            ("status", "active".to_string()),
        ],
    )?;
    let zone = zones
        .into_iter()
        .find(|zone| zone.name == zone_name)
        .with_context(|| format!("Cloudflare zone not found: {zone_name}"))?;

    let mut records: Vec<DnsRecord> =
        cloudflare.list_all(&format!("/zones/{}/dns_records", zone.id), &[])?;
    records.sort_by(|left, right| {
        format!(
            "{} {} {}",
            left.name,
            left.record_type,
            left.content.as_deref().unwrap_or_default()
        )
        .cmp(&format!(
            "{} {} {}",
            right.name,
            right.record_type,
            right.content.as_deref().unwrap_or_default()
        ))
    });

    write_zone_file(&zone)?;
    write_dns_records_file(&records)?;
    write_imports_file(&records, &zone.id)?;

    println!("Exported {} DNS records for {}.", records.len(), zone.name);
    Ok(())
}

fn write_zone_file(zone: &Zone) -> Result<()> {
    fs::write(
        "zone.generated.tf",
        format!(
            "# Generated by src/bin/export-dns.rs. Review before applying.\nlocals {{\n  zone_id   = {}\n  zone_name = {}\n}}\n",
            hcl_string(&zone.id),
            hcl_string(&zone.name)
        ),
    )
    .context("write zone.generated.tf")
}

fn write_dns_records_file(records: &[DnsRecord]) -> Result<()> {
    let mut output =
        String::from("# Generated by src/bin/export-dns.rs. Review before applying.\n");
    for record in records {
        output.push('\n');
        output.push_str(&dns_record_resource(record)?);
        output.push('\n');
    }
    fs::write("dns_records.generated.tf", output).context("write dns_records.generated.tf")
}

fn write_imports_file(records: &[DnsRecord], zone_id: &str) -> Result<()> {
    let mut output = String::from(
        "# Generated by src/bin/export-dns.rs. Keep until every record has been imported into local state.\n",
    );
    for record in records {
        output.push_str(&format!(
            "\nimport {{\n  to = cloudflare_dns_record.{}\n  id = {}\n}}\n",
            resource_name(record),
            hcl_string(&format!("{zone_id}/{}", record.id))
        ));
    }
    fs::write("imports.generated.tf", output).context("write imports.generated.tf")
}

fn dns_record_resource(record: &DnsRecord) -> Result<String> {
    let mut lines = vec![
        format!(
            "resource \"cloudflare_dns_record\" \"{}\" {{",
            resource_name(record)
        ),
        "  zone_id = local.zone_id".to_string(),
        format!("  name    = {}", hcl_string(&record.name)),
        format!("  type    = {}", hcl_string(&record.record_type)),
        format!("  ttl     = {}", record.ttl),
    ];

    if let Some(content) = record
        .content
        .as_deref()
        .filter(|content| !content.is_empty())
    {
        lines.push(format!("  content = {}", hcl_string(content)));
    }
    if let Some(proxied) = record.proxied {
        lines.push(format!("  proxied = {proxied}"));
    }
    if let Some(priority) = record.priority {
        lines.push(format!("  priority = {priority}"));
    }
    if let Some(comment) = record
        .comment
        .as_deref()
        .filter(|comment| !comment.is_empty())
    {
        lines.push(format!("  comment = {}", hcl_string(comment)));
    }
    if let Some(tags) = record.tags.as_ref().filter(|tags| !tags.is_empty()) {
        lines.push(format!("  tags = {}", hcl_json(&json!(tags))?));
    }
    if let Some(data) = record.data.as_ref().and_then(compact_json_value) {
        lines.push(format!("  data = {}", hcl_json(&data)?));
    }
    if let Some(settings) = record.settings.as_ref().and_then(compact_json_value) {
        lines.push(format!("  settings = {}", hcl_json(&settings)?));
    }

    lines.push(String::new());
    lines.push("  lifecycle {".to_string());
    lines.push("    prevent_destroy = true".to_string());
    lines.push("  }".to_string());
    lines.push("}".to_string());

    Ok(lines.join("\n"))
}

fn resource_name(record: &DnsRecord) -> String {
    let raw = format!("{}_{}", record.record_type, record.name).to_lowercase();
    let mut normalized = String::new();
    let mut previous_was_underscore = false;

    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch);
            previous_was_underscore = false;
        } else if !previous_was_underscore {
            normalized.push('_');
            previous_was_underscore = true;
        }
    }

    let trimmed = normalized.trim_matches('_');
    let safe = if trimmed.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        format!("r_{trimmed}")
    } else {
        trimmed.to_string()
    };

    format!("{}_{}", safe, &record.id[..8])
}

fn compact_json_value(value: &Value) -> Option<Value> {
    match value {
        Value::Object(map) => {
            let compacted: BTreeMap<String, Value> = map
                .iter()
                .filter_map(|(key, value)| {
                    if value.is_null() {
                        None
                    } else {
                        Some((key.clone(), value.clone()))
                    }
                })
                .collect();
            if compacted.is_empty() {
                None
            } else {
                Some(Value::Object(Map::from_iter(compacted)))
            }
        }
        Value::Null => None,
        other => Some(other.clone()),
    }
}

fn hcl_string(value: &str) -> String {
    serde_json::to_string(value).expect("serialize string")
}

fn hcl_json(value: &Value) -> Result<String> {
    serde_json::to_string_pretty(value).context("serialize JSON as HCL expression")
}
