//! IronClaw WASM tool: Gotify push notifications
//!
//! Uses IronClaw's host-provided http-request function.
//! Secrets (GOTIFY_APP_TOKEN) are injected by the host into
//! HTTP headers at the host boundary — never exposed to WASM.

wit_bindgen::generate!({
    world: "sandboxed-tool",
    path: "wit/tool.wit",
});

use serde::{Deserialize, Serialize};

use exports::near::agent::tool;

// ── Types ───────────────────────────────────────────────────────

#[derive(Deserialize)]
struct SendInput {
    #[serde(default = "default_title")]
    title: String,
    message: String,
    #[serde(default = "default_priority")]
    priority: i32,
}

fn default_title() -> String {
    "Kageho".to_string()
}

fn default_priority() -> i32 {
    9
}

#[derive(Serialize)]
struct GotifyMessage {
    title: String,
    message: String,
    priority: i32,
}

// ── Tool implementation ─────────────────────────────────────────

struct GotifyTool;

export!(GotifyTool);

impl tool::Guest for GotifyTool {
    fn execute(req: tool::Request) -> tool::Response {
        let result = dispatch(&req.params);
        match result {
            Ok(output) => tool::Response {
                output: Some(output),
                error: None,
            },
            Err(e) => tool::Response {
                output: None,
                error: Some(e),
            },
        }
    }

    fn schema() -> String {
        r#"{
  "type": "object",
  "properties": {
    "title": {
      "type": "string",
      "description": "Notification title. Defaults to 'Kageho'."
    },
    "message": {
      "type": "string",
      "description": "Notification body text. Supports markdown."
    },
    "priority": {
      "type": "integer",
      "description": "Priority: 1-3=low, 5-7=medium, 8-10=high. Default 3.",
      "default": 3
    }
  },
  "required": ["message"]
}"#
        .to_string()
    }

    fn description() -> String {
        "Send a push notification via Gotify. Use this to notify the user about completed tasks, alerts, reminders, or any important information.".to_string()
    }
}

// ── Logic ───────────────────────────────────────────────────────

fn dispatch(params_json: &str) -> Result<String, String> {
    let params: SendInput =
        serde_json::from_str(params_json).map_err(|e| format!("Bad input: {e}"))?;

    // Check secrets exist in the encrypted vault
    if !near::agent::host::secret_exists("gotify_app_token") {
        return Err("Secret 'gotify_app_token' not configured. Run: python3 insert_secret.py gotify_app_token <token>".into());
    }

    let msg = GotifyMessage {
        title: params.title,
        message: params.message,
        priority: params.priority,
    };

    let body = serde_json::to_string(&msg).map_err(|e| format!("JSON error: {e}"))?;

    near::agent::host::log(
        near::agent::host::LogLevel::Info,
        &format!("Sending Gotify notification: {}", msg.title),
    );

    // Host injects the real token from the encrypted vault
    let headers = serde_json::json!({
        "Content-Type": "application/json"
    });
    // Host replaces ${gotify_url} with the decrypted value
    //let url = "${gotify_url}/message";
    let url = "https://gotify.darkc.sobe.world/message";
    let response = near::agent::host::http_request(
        "POST",
        url,
        &headers.to_string(),
        Some(body.as_bytes()),
        Some(10000),
    )
    .map_err(|e| format!("HTTP failed: {e}"))?;

    let status = response.status;
    let resp_body = String::from_utf8_lossy(&response.body).to_string();

    if status >= 200 && status < 300 {
        near::agent::host::log(
            near::agent::host::LogLevel::Info,
            &format!("Gotify notification sent (HTTP {status})"),
        );
        Ok(format!(
            "{{\"success\":true,\"message\":\"Notification sent (HTTP {status})\"}}"
        ))
    } else {
        Err(format!("Gotify returned HTTP {status}: {resp_body}"))
    }
}
