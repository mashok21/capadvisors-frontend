use serde::{Deserialize, Serialize};

// ── Public result type ────────────────────────────────────────────────────────

/// Structured output from the LLM curriculum gap-analysis pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapAnalysisResult {
    /// 0.0 – 1.0 overall coverage of the chapter's learning objectives.
    pub coverage_score: f64,
    /// Topics present in the chapter syllabus but absent or under-covered in
    /// the uploaded document.
    pub gap_topics: Vec<String>,
    /// Topics that are adequately addressed by the uploaded material.
    pub compliant_topics: Vec<String>,
    /// Concrete study recommendations for bridging the identified gaps.
    pub recommendations: Vec<String>,
}

// ── OpenAI chat completion shapes ────────────────────────────────────────────

#[derive(Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: &'static str,
    messages: Vec<ChatMessage>,
    response_format: serde_json::Value,
    temperature: f64,
}

#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Sends the semantically-retrieved chunks to GPT-4o-mini with a structured
/// curriculum gap-analysis prompt and returns a parsed [`GapAnalysisResult`].
///
/// Uses `response_format: { "type": "json_object" }` so the model is
/// constrained to emit valid JSON and never wrap it in markdown fences.
///
/// Reads `OPENAI_API_KEY` from the environment at call time.
pub async fn analyse_gaps(
    client: &reqwest::Client,
    chapter_name: &str,
    retrieved_chunks: &[String],
) -> Result<GapAnalysisResult, String> {
    let api_key = std::env::var("OPENAI_API_KEY")
        .map_err(|_| "OPENAI_API_KEY environment variable is not set".to_string())?;

    let context = retrieved_chunks.join("\n\n---\n\n");

    let system_content = "\
You are an expert CA Final AFM (Advanced Financial Management) curriculum analyst. \
Given the name of a target chapter and retrieved excerpts from a student's uploaded \
study material, produce a JSON gap analysis. \
\n\nRespond ONLY with a single JSON object with these exact keys:\
\n  coverage_score  – float between 0 and 1 indicating overall syllabus coverage\
\n  gap_topics      – array of strings: topics in the chapter not covered by the material\
\n  compliant_topics – array of strings: topics adequately covered\
\n  recommendations – array of strings: specific study actions to close the gaps\
\n\nDo NOT wrap the JSON in markdown fences or add any prose outside the object."
        .to_string();

    let user_content = format!(
        "Target chapter: {chapter_name}\n\nRetrieved study material:\n\"\"\"\n{context}\n\"\"\"\n\n\
         Analyse the coverage and return the JSON gap analysis."
    );

    let req = ChatRequest {
        model: "gpt-4o-mini",
        messages: vec![
            ChatMessage { role: "system", content: system_content },
            ChatMessage { role: "user",   content: user_content },
        ],
        response_format: serde_json::json!({ "type": "json_object" }),
        temperature: 0.2,
    };

    let res = client
        .post("https://api.openai.com/v1/chat/completions")
        .bearer_auth(&api_key)
        .json(&req)
        .send()
        .await
        .map_err(|e| format!("LLM network error: {}", e))?;

    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        return Err(format!("OpenAI chat API {} — {}", status, body));
    }

    let chat: ChatResponse = res
        .json()
        .await
        .map_err(|e| format!("Failed to deserialise LLM response: {}", e))?;

    let content = chat
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .ok_or_else(|| "OpenAI returned an empty choices array".to_string())?;

    serde_json::from_str::<GapAnalysisResult>(&content).map_err(|e| {
        format!(
            "Failed to parse GapAnalysisResult JSON: {}. Raw content: {}",
            e, content
        )
    })
}
