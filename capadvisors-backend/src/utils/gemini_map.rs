use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// Structured output types — must mirror the responseSchema sent to Gemini
// ─────────────────────────────────────────────────────────────────────────────

/// Top-level structured response from the Gemini 2.5 Pro map-analysis pass.
///
/// `deny_unknown_fields` is intentionally omitted here: if Gemini ever emits
/// a minor extra key at the top level (e.g. a metadata field), the response
/// should still deserialise cleanly rather than hard-failing. Unknown fields
/// on the tightly-constrained inner types (ScoringRubric, DiagnosticVariant)
/// ARE rejected — see their own derive annotations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiMapResult {
    /// Integer 0–100 representing compliance depth relative to chapter syllabus.
    pub coverage_metric: i64,
    /// Ordered step-list of mathematical proofs and formula derivations verified
    /// by the model to prevent calculation hallucinations.
    pub computational_checks: Vec<String>,
    /// Fully framed high-stakes ICAI CA Final AFM case scenario problem derived
    /// strictly from the uploaded document.
    pub complex_exam_question: String,
    /// Step-by-step fractional marks breakdown for the case scenario.
    /// Field name matches the JSON key Gemini emits.
    pub scoring_rubric_json: ScoringRubric,
    /// Exactly three diagnostic permutations of the main question targeting
    /// specific cognitive blind spots.
    pub alternate_diagnostic_variants: Vec<DiagnosticVariant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoringRubric {
    pub total_marks: i64,
    pub steps: Vec<RubricStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RubricStep {
    pub step: i64,
    pub description: String,
    /// May be fractional (e.g. 0.5 for partial credit).
    pub marks: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticVariant {
    /// 1, 2, or 3.
    pub variant_number: i64,
    /// `NUMERICAL_SHIFT` | `STRATEGIC_REVERSE` | `EDGE_CASE_LIMIT`
    pub variant_type: String,
    pub title: String,
    pub question: String,
    /// Precise description of what differs from the main question.
    pub key_difference: String,
}

impl GeminiMapResult {
    /// Validates the structural integrity of the Gemini response before it is
    /// persisted.  Returns `Err` with a human-readable diagnosis on failure.
    ///
    /// Checks:
    /// 1. The sum of individual rubric step marks matches `total_marks`
    ///    (within a 0.5-mark tolerance for floating-point rounding).
    /// 2. `alternate_diagnostic_variants` contains at most 3 items.
    pub fn validate(&self) -> Result<(), String> {
        let computed_sum: f64 = self.scoring_rubric_json.steps.iter().map(|s| s.marks).sum();
        let expected = self.scoring_rubric_json.total_marks as f64;
        if (computed_sum - expected).abs() > 0.5 {
            return Err(format!(
                "Rubric step marks sum to {:.2} but total_marks is {} \
                 (delta {:.2} exceeds 0.5-mark tolerance)",
                computed_sum,
                self.scoring_rubric_json.total_marks,
                (computed_sum - expected).abs()
            ));
        }
        let variant_count = self.alternate_diagnostic_variants.len();
        if variant_count > 3 {
            return Err(format!(
                "Expected at most 3 diagnostic variants, Gemini returned {}",
                variant_count
            ));
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Gemini API client
// ─────────────────────────────────────────────────────────────────────────────

const GEMINI_ENDPOINT: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-pro:generateContent";

/// Sends the full document text to Gemini 2.5 Pro and returns a validated
/// [`GeminiMapResult`].
///
/// The document is sent **as a single unified context** — no chunking — so
/// that formulas spanning page boundaries (Black-Scholes, APV calculations,
/// multi-step Forex hedging chains) are always processed in full.
///
/// Reads `GEMINI_API_KEY` from the environment at call time.
/// Applies a 300-second per-request timeout via `RequestBuilder::timeout`.
pub async fn call_gemini_map(
    client: &reqwest::Client,
    chapter_name: &str,
    chapter_code: &str,
    raw_document_text: &str,
) -> Result<GeminiMapResult, String> {
    let api_key = std::env::var("GEMINI_API_KEY")
        .map_err(|_| "GEMINI_API_KEY environment variable is not set".to_string())?;

    let url = format!("{}?key={}", GEMINI_ENDPOINT, api_key);

    let prompt = build_prompt(chapter_code, chapter_name, raw_document_text);
    let schema = build_response_schema();

    let payload = serde_json::json!({
        "contents": [
            {
                "parts": [ { "text": prompt } ]
            }
        ],
        "generationConfig": {
            "responseMimeType": "application/json",
            "responseSchema": schema,
            // Low temperature for mathematical accuracy; small top-p for
            // deterministic formula derivations.
            "temperature": 0.3,
            "topP": 0.85
        }
    });

    let res = client
        .post(&url)
        .json(&payload)
        .timeout(std::time::Duration::from_secs(300))
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

    // Gemini structured-output JSON is embedded inside the text part, even
    // when responseMimeType is application/json — same extraction path as the
    // existing nexus.rs Gemini integration.
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

// ─────────────────────────────────────────────────────────────────────────────
// Private helpers
// ─────────────────────────────────────────────────────────────────────────────

fn build_prompt(chapter_code: &str, chapter_name: &str, raw_text: &str) -> String {
    format!(
        "You are an expert ICAI CA Final Advanced Financial Management (AFM) examiner \
         and curriculum auditor specialising in {chapter_code}: {chapter_name}.\n\n\
         You are given the COMPLETE, UNTRUNCATED text of a student's study document. \
         Analyse every section in full — do not skip, summarise, or omit any portion.\n\
         Pay particular attention to:\n\
         • Mathematical formulae and their derivations (Black-Scholes, CAPM, APV, \
           Forex hedging, Securitization waterfalls, IRR/MIRR chains)\n\
         • Step-by-step numerical examples and their intermediate calculations\n\
         • Definitions, conditions, and edge-case caveats stated in the text\n\n\
         Based on your analysis, return a single JSON object matching the required schema.\n\
         The `alternate_diagnostic_variants` array MUST contain EXACTLY 3 items:\n\
         - Variant 1 (NUMERICAL_SHIFT): change a key input value to test compounding/rounding\n\
         - Variant 2 (STRATEGIC_REVERSE): invert the strategic objective or direction\n\
         - Variant 3 (EDGE_CASE_LIMIT): probe a boundary condition or rule exception\n\n\
         DOCUMENT TEXT (analyse in full):\n\
         \"\"\"\n\
         {raw_text}\n\
         \"\"\""
    )
}

pub(crate) fn build_response_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "OBJECT",
        "properties": {
            "coverage_metric": {
                "type": "INTEGER",
                "description": "Integer 0-100: depth of curriculum compliance for this chapter"
            },
            "computational_checks": {
                "type": "ARRAY",
                "description": "Ordered list of mathematical proofs and formula derivations \
                                verified, confirming zero calculation hallucinations",
                "items": { "type": "STRING" }
            },
            "complex_exam_question": {
                "type": "STRING",
                "description": "Fully framed, high-stakes ICAI-pattern CA Final AFM case \
                                scenario problem derived strictly from the document content"
            },
            "scoring_rubric_json": {
                "type": "OBJECT",
                "description": "Step-by-step mark allocation showing how fractional marks \
                                are awarded for each intermediate calculation",
                "properties": {
                    "total_marks": {
                        "type": "INTEGER",
                        "description": "Total marks for the case scenario (typically 20 or 25)"
                    },
                    "steps": {
                        "type": "ARRAY",
                        "items": {
                            "type": "OBJECT",
                            "properties": {
                                "step":        { "type": "INTEGER" },
                                "description": { "type": "STRING" },
                                "marks":       {
                                    "type": "NUMBER",
                                    "description": "Marks for this step; may be fractional (0.5)"
                                }
                            },
                            "required": ["step", "description", "marks"]
                        }
                    }
                },
                "required": ["total_marks", "steps"]
            },
            "alternate_diagnostic_variants": {
                "type": "ARRAY",
                "description": "Exactly 3 diagnostic permutations of the main question",
                "items": {
                    "type": "OBJECT",
                    "properties": {
                        "variant_number": {
                            "type": "INTEGER",
                            "description": "1, 2, or 3"
                        },
                        "variant_type": {
                            "type": "STRING",
                            "description": "NUMERICAL_SHIFT | STRATEGIC_REVERSE | EDGE_CASE_LIMIT"
                        },
                        "title": {
                            "type": "STRING",
                            "description": "Short descriptive title for the variant"
                        },
                        "question": {
                            "type": "STRING",
                            "description": "The modified question text"
                        },
                        "key_difference": {
                            "type": "STRING",
                            "description": "One sentence explaining exactly what was changed and why"
                        }
                    },
                    "required": ["variant_number", "variant_type", "title", "question", "key_difference"]
                }
            }
        },
        "required": [
            "coverage_metric",
            "computational_checks",
            "complex_exam_question",
            "scoring_rubric_json",
            "alternate_diagnostic_variants"
        ]
    })
}
