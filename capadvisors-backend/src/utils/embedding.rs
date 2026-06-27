use serde::{Deserialize, Serialize};

/// Dimensionality of the embedding vectors produced by this module.
/// Must match the `F32_BLOB(N)` dimension in the `chunk_embeddings` DDL.
pub const EMBEDDING_DIM: usize = 768;

const EMBEDDING_MODEL: &str = "text-embedding-3-small";

// ── OpenAI request / response shapes ─────────────────────────────────────────

#[derive(Serialize)]
struct EmbedRequest<'a> {
    model: &'static str,
    input: &'a str,
    dimensions: usize,
}

#[derive(Deserialize)]
struct EmbedData {
    embedding: Vec<f32>,
}

#[derive(Deserialize)]
struct EmbedResponse {
    data: Vec<EmbedData>,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Calls the OpenAI embeddings endpoint and returns a `Vec<f32>` of length
/// [`EMBEDDING_DIM`].
///
/// Reads `OPENAI_API_KEY` from the environment at call time.
/// Returns `Err` if the key is missing, the network request fails, or the
/// API responds with a non-2xx status.
pub async fn embed_text(client: &reqwest::Client, text: &str) -> Result<Vec<f32>, String> {
    let api_key = std::env::var("OPENAI_API_KEY")
        .map_err(|_| "OPENAI_API_KEY environment variable is not set".to_string())?;

    let body = EmbedRequest {
        model: EMBEDDING_MODEL,
        input: text,
        dimensions: EMBEDDING_DIM,
    };

    let res = client
        .post("https://api.openai.com/v1/embeddings")
        .bearer_auth(&api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Embedding network error: {}", e))?;

    if !res.status().is_success() {
        let status = res.status();
        let body_text = res.text().await.unwrap_or_default();
        return Err(format!("OpenAI embeddings API {} — {}", status, body_text));
    }

    let parsed: EmbedResponse = res
        .json()
        .await
        .map_err(|e| format!("Failed to deserialise embedding response: {}", e))?;

    parsed
        .data
        .into_iter()
        .next()
        .map(|d| d.embedding)
        .ok_or_else(|| "OpenAI returned an empty embedding data array".to_string())
}

/// Serialises a `Vec<f32>` to the JSON array string that libSQL's `vector32()`
/// function accepts, e.g. `"[0.1,-0.3,...]"`.
///
/// Non-finite floats (NaN / Inf) are clamped to `0.0` so the database call
/// never receives an invalid JSON number.
pub fn vec_to_json(v: &[f32]) -> String {
    let inner = v
        .iter()
        .map(|x| {
            if x.is_finite() {
                x.to_string()
            } else {
                "0".to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{}]", inner)
}
