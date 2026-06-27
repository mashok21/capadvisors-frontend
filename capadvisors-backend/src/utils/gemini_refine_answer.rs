use crate::utils::gemini_map::ScoringRubric;

const GEMINI_ENDPOINT: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-pro:generateContent";

// Verbatim system instruction required by spec — loaded once as a static str
// rather than heap-allocated on every call.
const SYSTEM_INSTRUCTION: &str =
    "You are an expert ICAI chief examiner auditing solutions for the CA Final AFM exam. \
     You are given an existing case study question and its current grading rubric. \
     Your task is to refine, correct, or polish the step-by-step answer rubric based \
     strictly on the admin's guidance (e.g., displaying formulas in clear LaTeX formatting, \
     correcting intermediate rounding errors, or breaking down marks into clear fractional \
     components). You MUST NOT alter the underlying question narrative text. \
     Output must rigidly match the ScoringRubric JSON schema.";

/// Passes an *existing* staged question's rubric to Gemini 2.5 Pro for surgical
/// refinement without touching the question narrative or diagnostic variants.
///
/// Temperature 0.1 is intentional: the model must audit mathematical steps, not
/// generate creative content. This minimises hallucinated intermediate values while
/// still allowing LaTeX/formatting transformations under admin guidance.
///
/// Returns only the [`ScoringRubric`] — the caller is responsible for persisting
/// it and re-fetching the full staging row.
pub async fn refine_question_answer(
    client: &reqwest::Client,
    question_text: &str,
    current_rubric_json: &str,
    admin_guidance: Option<&str>,
) -> Result<ScoringRubric, String> {
    let api_key = std::env::var("GEMINI_API_KEY")
        .map_err(|_| "GEMINI_API_KEY environment variable is not set".to_string())?;

    let url = format!("{}?key={}", GEMINI_ENDPOINT, api_key);

    let payload = serde_json::json!({
        "systemInstruction": {
            "parts": [{ "text": SYSTEM_INSTRUCTION }]
        },
        "contents": [
            {
                "parts": [{
                    "text": build_refine_prompt(question_text, current_rubric_json, admin_guidance)
                }]
            }
        ],
        "generationConfig": {
            "responseMimeType": "application/json",
            "responseSchema": build_rubric_schema(),
            "temperature": 0.1,
            "topP": 0.80
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

    serde_json::from_str::<ScoringRubric>(text).map_err(|e| {
        format!(
            "Failed to deserialise ScoringRubric: {}. Raw response: {}",
            e, text
        )
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Private helpers
// ─────────────────────────────────────────────────────────────────────────────

fn build_refine_prompt(
    question_text: &str,
    current_rubric_json: &str,
    admin_guidance: Option<&str>,
) -> String {
    let guidance_section = match admin_guidance {
        Some(g) if !g.trim().is_empty() => format!(
            "ADMIN GUIDANCE (apply precisely — this is a directed refinement):\n\
             \"{}\"\n\n",
            g.trim()
        ),
        _ => "ADMIN GUIDANCE: None provided. Auto-polish only — improve LaTeX formula \
              notation, fix intermediate rounding inconsistencies, and clarify step \
              descriptions for examiner readability. Do NOT alter mark allocations or \
              restructure the rubric steps.\n\n"
            .to_string(),
    };

    format!(
        "{guidance_section}\
         QUESTION TEXT (read-only — do NOT reproduce or modify this):\n\
         \"\"\"\n{question_text}\n\"\"\"\n\n\
         CURRENT SCORING RUBRIC (JSON — refine this and return the improved version):\n\
         {current_rubric_json}\n\n\
         Return only the refined ScoringRubric as a JSON object. \
         Constraints:\n\
         • total_marks MUST remain unchanged unless the admin guidance explicitly \
           instructs a mark reallocation.\n\
         • The sum of all step marks must equal total_marks within ±0.5 tolerance.\n\
         • Step numbers must remain sequential starting from 1.\n\
         • Every step must retain a `step`, `description`, and `marks` field.",
        guidance_section = guidance_section,
        question_text = question_text,
        current_rubric_json = current_rubric_json,
    )
}

/// Narrower schema than the full `GeminiMapResult` schema — only the rubric
/// sub-object is requested, which halves the output token budget and reduces
/// the risk of the model drifting into question-text territory.
fn build_rubric_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "OBJECT",
        "properties": {
            "total_marks": {
                "type": "INTEGER",
                "description": "Total marks for the question (unchanged from original \
                                unless guidance explicitly mandates reallocation)"
            },
            "steps": {
                "type": "ARRAY",
                "description": "Step-by-step mark allocation. Each step must have a \
                                clear formula/calculation description and its partial mark.",
                "items": {
                    "type": "OBJECT",
                    "properties": {
                        "step": {
                            "type": "INTEGER",
                            "description": "Sequential step number starting from 1"
                        },
                        "description": {
                            "type": "STRING",
                            "description": "Full examiner-facing step description, \
                                            including LaTeX-formatted formulas where applicable"
                        },
                        "marks": {
                            "type": "NUMBER",
                            "description": "Marks for this step; may be fractional (e.g. 0.5)"
                        }
                    },
                    "required": ["step", "description", "marks"]
                }
            }
        },
        "required": ["total_marks", "steps"]
    })
}
