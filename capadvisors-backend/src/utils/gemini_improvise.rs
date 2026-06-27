use crate::utils::gemini_map::{build_response_schema, GeminiMapResult};

const GEMINI_ENDPOINT: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-pro:generateContent";

/// Passes an *existing* staged question back to Gemini for polish and
/// refinement, skipping the full document-parsing pipeline entirely.
///
/// The model operates at temperature 0.2 (versus 0.3 for full mapping) so
/// it preserves numerical values and formula logic by default, only applying
/// directed changes when `admin_guidance` specifies them.
pub async fn improvise_question(
    client: &reqwest::Client,
    current_question: &str,
    current_rubric_json: &str,
    current_variants_json: &str,
    admin_guidance: Option<&str>,
) -> Result<GeminiMapResult, String> {
    let api_key = std::env::var("GEMINI_API_KEY")
        .map_err(|_| "GEMINI_API_KEY environment variable is not set".to_string())?;

    let url = format!("{}?key={}", GEMINI_ENDPOINT, api_key);

    let prompt = build_improvise_prompt(
        current_question,
        current_rubric_json,
        current_variants_json,
        admin_guidance,
    );

    let payload = serde_json::json!({
        "contents": [
            {
                "parts": [ { "text": prompt } ]
            }
        ],
        "generationConfig": {
            "responseMimeType": "application/json",
            "responseSchema": build_response_schema(),
            "temperature": 0.2,
            "topP": 0.85
        }
    });

    let res = client
        .post(&url)
        .json(&payload)
        .timeout(std::time::Duration::from_secs(90))
        .send()
        .await
        .map_err(|e| format!("Gemini network error: {}", e))?;

    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        return Err(format!("Gemini API {} — {}", status, body));
    }

    let envelope: serde_json::Value = res
        .json()
        .await
        .map_err(|e| format!("Failed to parse Gemini response envelope: {}", e))?;

    let text = envelope["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .ok_or_else(|| {
            format!(
                "Unexpected Gemini response shape: {}",
                serde_json::to_string_pretty(&envelope).unwrap_or_default()
            )
        })?;

    serde_json::from_str::<GeminiMapResult>(text).map_err(|e| {
        format!(
            "Failed to deserialise GeminiMapResult: {}. Raw text: {}",
            e, text
        )
    })
}

fn build_improvise_prompt(
    current_question: &str,
    current_rubric_json: &str,
    current_variants_json: &str,
    admin_guidance: Option<&str>,
) -> String {
    let guidance_section = match admin_guidance {
        Some(g) if !g.trim().is_empty() => format!(
            "ADMIN GUIDANCE (apply precisely — this is a directed change):\n\"{}\"\n\n",
            g.trim()
        ),
        _ => "ADMIN GUIDANCE: None provided. Auto-polish only — fix grammar, \
              clean mathematical notation, improve narrative clarity. \
              Do NOT alter any numbers or formula structure.\n\n"
            .to_string(),
    };

    format!(
        "You are an expert ICAI exam designer and CA Final AFM question specialist.\n\n\
         You are given an EXISTING high-stakes case question, its step-by-step grading \
         rubric, and its three diagnostic variants. Your sole task is to polish and refine \
         them according to the admin guidance below.\n\n\
         CONSTRAINTS (non-negotiable):\n\
         • You MUST maintain all original numerical values and formula logic unless the \
           admin guidance explicitly instructs you to change a specific value.\n\
         • Do NOT restructure the problem type or invent new question components.\n\
         • All three diagnostic variants must be preserved and improved in-place.\n\
         • The output JSON MUST rigidly match the required GeminiMapResult schema.\n\n\
         {guidance_section}\
         CURRENT QUESTION:\n\
         \"\"\"\n{current_question}\n\"\"\"\n\n\
         CURRENT SCORING RUBRIC (JSON):\n\
         {current_rubric_json}\n\n\
         CURRENT DIAGNOSTIC VARIANTS (JSON):\n\
         {current_variants_json}\n\n\
         Return the complete refined question set as a single JSON object. \
         Set coverage_metric to reflect the polished question's curriculum depth (0-100). \
         List the key mathematical verification steps in computational_checks.",
        guidance_section = guidance_section,
        current_question = current_question,
        current_rubric_json = current_rubric_json,
        current_variants_json = current_variants_json,
    )
}
