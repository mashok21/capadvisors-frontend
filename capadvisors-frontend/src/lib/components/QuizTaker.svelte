<script>
  import { auth } from '../auth.svelte.js';
  import { CHAPTERS } from '../chapters.js';

  let { baseApiUrl = '' } = $props();

  const COUNT_OPTIONS = [5, 10, 15, 20];

  // 'setup' | 'active' | 'results'
  let stage = $state('setup');

  // Setup form state
  let selectedChapterCode = $state('');
  let questionCount = $state(10);

  // Active quiz state
  let quiz = $state(null);            // GeneratedQuiz payload from the backend
  let answers = $state({});           // question_id -> selected option text
  let isGenerating = $state(false);
  let isSubmitting = $state(false);
  let error = $state('');

  // Results state
  let result = $state(null);          // QuizSubmitResponse payload

  let answeredCount = $derived(Object.keys(answers).length);
  let totalQuestions = $derived(quiz?.questions?.length ?? 0);

  function authHeaders() {
    return {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${auth.token}`,
    };
  }

  async function startQuiz() {
    isGenerating = true;
    error = '';
    try {
      const params = new URLSearchParams();
      if (selectedChapterCode) params.set('chapter_id', selectedChapterCode);
      params.set('count', String(questionCount));

      const res = await fetch(`${baseApiUrl}/api/quizzes/generate?${params}`, {
        headers: authHeaders(),
      });
      if (!res.ok) throw new Error(await res.text() || `HTTP ${res.status}`);

      quiz = await res.json();
      answers = {};
      result = null;
      stage = 'active';
    } catch (e) {
      error = `Could not generate quiz: ${e.message}`;
    } finally {
      isGenerating = false;
    }
  }

  function selectOption(questionId, option) {
    answers = { ...answers, [questionId]: option };
  }

  async function submitQuiz() {
    if (!quiz || answeredCount === 0) return;
    isSubmitting = true;
    error = '';
    try {
      const payload = {
        quiz_id: quiz.quiz_id,
        answers: Object.entries(answers).map(([question_id, selected_option]) => ({
          question_id,
          selected_option,
        })),
      };

      const res = await fetch(`${baseApiUrl}/api/quizzes/submit`, {
        method: 'POST',
        headers: authHeaders(),
        body: JSON.stringify(payload),
      });
      if (!res.ok) throw new Error(await res.text() || `HTTP ${res.status}`);

      result = await res.json();
      stage = 'results';
    } catch (e) {
      error = `Could not submit quiz: ${e.message}`;
    } finally {
      isSubmitting = false;
    }
  }

  function retakeQuiz() {
    stage = 'setup';
    quiz = null;
    answers = {};
    result = null;
    error = '';
  }
</script>

<div class="qt-root">
  {#if error}
    <div class="qt-error">{error}</div>
  {/if}

  <!-- ── Setup ─────────────────────────────────────────────────────────── -->
  {#if stage === 'setup'}
    <div class="qt-setup">
      <div class="qt-setup-header">
        <h2 class="qt-title">Adaptive Practice Quiz</h2>
        <p class="qt-subtitle">Questions are matched to your current Glicko-2 rating band — 60% zone-matched, 20% easier, 20% harder.</p>
      </div>

      <div class="qt-form-row">
        <label class="qt-label" for="qt-chapter">Chapter Focus</label>
        <select id="qt-chapter" class="qt-select" bind:value={selectedChapterCode}>
          <option value="">All Chapters</option>
          {#each CHAPTERS as ch}
            <option value={ch.chapter_code}>{ch.chapter_code}: {ch.chapter_name}</option>
          {/each}
        </select>
      </div>

      <div class="qt-form-row">
        <span class="qt-label">Question Count</span>
        <div class="qt-count-group">
          {#each COUNT_OPTIONS as n}
            <button
              type="button"
              class="qt-count-btn {questionCount === n ? 'selected' : ''}"
              onclick={() => questionCount = n}
            >{n}</button>
          {/each}
        </div>
      </div>

      <button class="qt-start-btn" onclick={startQuiz} disabled={isGenerating}>
        {#if isGenerating}
          <span class="spinner"></span> Generating quiz...
        {:else}
          🚀 Start Quiz
        {/if}
      </button>
    </div>
  {/if}

  <!-- ── Active quiz ───────────────────────────────────────────────────── -->
  {#if stage === 'active' && quiz}
    <div class="qt-active">
      <div class="qt-active-header">
        <div class="qt-meta">
          <span class="qt-meta-item">Rating: <strong>{quiz.student_rating.toFixed(0)}</strong> ± {quiz.student_rd.toFixed(0)}</span>
          <span class="qt-meta-item qt-band-label">{quiz.difficulty_band.label} Band</span>
        </div>
        <span class="qt-progress-text">{answeredCount} / {totalQuestions} answered</span>
      </div>
      <div class="qt-progress-bar">
        <div class="qt-progress-fill" style="width: {totalQuestions ? (answeredCount / totalQuestions) * 100 : 0}%"></div>
      </div>

      <div class="qt-questions-list">
        {#each quiz.questions as question, index}
          <div class="qt-question-block">
            <div class="qt-q-header">
              <span class="qt-q-number">Question {index + 1}</span>
              <span class="qt-q-tags">
                <span class="qt-q-chapter">{question.chapter_code}</span>
                <span class="qt-q-difficulty {question.difficulty.toLowerCase()}">{question.difficulty}</span>
              </span>
            </div>

            <p class="qt-q-scenario">{question.scenario}</p>

            <div class="qt-q-options">
              {#each question.options as option}
                <button
                  type="button"
                  class="qt-option-row {answers[question.id] === option ? 'selected' : ''}"
                  onclick={() => selectOption(question.id, option)}
                >
                  <span class="qt-option-dot"></span>
                  <span class="qt-option-text">{option}</span>
                </button>
              {/each}
            </div>
          </div>
        {/each}
      </div>

      <div class="qt-submit-bar">
        {#if answeredCount < totalQuestions}
          <span class="qt-submit-hint">{totalQuestions - answeredCount} question{totalQuestions - answeredCount !== 1 ? 's' : ''} left unanswered — only answered questions count toward your score.</span>
        {/if}
        <button class="qt-submit-btn" onclick={submitQuiz} disabled={isSubmitting || answeredCount === 0}>
          {#if isSubmitting}
            <span class="spinner"></span> Scoring submission...
          {:else}
            ✅ Submit Quiz
          {/if}
        </button>
      </div>
    </div>
  {/if}

  <!-- ── Results ───────────────────────────────────────────────────────── -->
  {#if stage === 'results' && result}
    <div class="qt-results">
      <h2 class="qt-title">Scorecard</h2>

      {#if result.is_ai_flagged}
        <div class="qt-ai-flag-banner">
          ⚠️ {result.penalty_points_applied} penalty point{result.penalty_points_applied !== 1 ? 's' : ''} applied — one or more answers were flagged as AI-generated.
        </div>
      {/if}

      <div class="qt-results-grid">
        <div class="qt-result-card">
          <span class="qt-result-label">Rating Change</span>
          <span class="qt-result-value {result.rating_change >= 0 ? 'positive' : 'negative'}">
            {result.rating_change >= 0 ? '▲' : '▼'} {Math.abs(result.rating_change).toFixed(0)}
          </span>
          <span class="qt-result-sub">{result.old_rating.toFixed(0)} → {result.new_rating.toFixed(0)}</span>
        </div>

        <div class="qt-result-card">
          <span class="qt-result-label">Rank Tier</span>
          <span class="qt-result-value">{result.rank_tier}</span>
        </div>

        <div class="qt-result-card">
          <span class="qt-result-label">Scorecard Total</span>
          <span class="qt-result-value">{result.final_scorecard_total}</span>
          <span class="qt-result-sub">{result.questions_evaluated} evaluated{result.questions_skipped ? `, ${result.questions_skipped} skipped` : ''}</span>
        </div>

        <div class="qt-result-card">
          <span class="qt-result-label">Rating Deviation</span>
          <span class="qt-result-value">±{result.new_rating_deviation.toFixed(0)}</span>
          <span class="qt-result-sub">was ±{result.old_rating_deviation.toFixed(0)}</span>
        </div>
      </div>

      <button class="qt-start-btn" onclick={retakeQuiz}>🔁 Take Another Quiz</button>
    </div>
  {/if}
</div>

<style>
  .qt-root {
    width: 100%;
    background: rgba(255, 255, 255, 0.018);
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 16px;
    padding: 28px 32px;
    box-sizing: border-box;
  }

  .qt-error {
    padding: 14px 18px;
    background: rgba(255, 80, 80, 0.08);
    border: 1px solid rgba(255, 80, 80, 0.2);
    border-radius: 10px;
    color: #ff8080;
    font-size: 0.85rem;
    margin-bottom: 20px;
  }

  .qt-title { font-size: 1.3rem; font-weight: 700; color: #f0f0ff; margin: 0 0 3px; letter-spacing: -0.01em; }
  .qt-subtitle { font-size: 0.8rem; color: rgba(255, 255, 255, 0.4); margin: 0 0 24px; }

  /* ── Setup ─────────────────────────────────────────────────────────── */
  .qt-setup-header { margin-bottom: 24px; }
  .qt-form-row { display: flex; flex-direction: column; gap: 8px; margin-bottom: 20px; max-width: 480px; }
  .qt-label { font-size: 0.78rem; font-weight: 600; color: rgba(255, 255, 255, 0.55); }
  .qt-select {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.12);
    color: #e8e8f0;
    border-radius: 8px;
    padding: 10px 12px;
    font-size: 0.85rem;
  }
  .qt-count-group { display: flex; gap: 8px; }
  .qt-count-btn {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.6);
    border-radius: 8px;
    padding: 8px 16px;
    font-size: 0.85rem;
    cursor: pointer;
    transition: all 0.15s;
  }
  .qt-count-btn:hover { background: rgba(255, 255, 255, 0.09); color: #fff; }
  .qt-count-btn.selected {
    background: rgba(96, 165, 250, 0.14);
    border-color: rgba(96, 165, 250, 0.4);
    color: #60a5fa;
  }

  .qt-start-btn {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    background: linear-gradient(135deg, #60a5fa, #34d399);
    border: none;
    color: #0c0e1a;
    font-weight: 700;
    border-radius: 10px;
    padding: 12px 24px;
    font-size: 0.9rem;
    cursor: pointer;
    transition: filter 0.15s, transform 0.15s;
  }
  .qt-start-btn:hover:not(:disabled) { filter: brightness(1.08); transform: translateY(-1px); }
  .qt-start-btn:disabled { opacity: 0.5; cursor: not-allowed; }

  /* ── Active quiz ───────────────────────────────────────────────────── */
  .qt-active-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 10px;
    margin-bottom: 10px;
  }
  .qt-meta { display: flex; align-items: center; gap: 10px; }
  .qt-meta-item { font-size: 0.82rem; color: rgba(255, 255, 255, 0.5); }
  .qt-meta-item strong { color: #f0f0ff; }
  .qt-band-label {
    background: rgba(240, 192, 64, 0.12);
    border: 1px solid rgba(240, 192, 64, 0.3);
    color: #f0c040;
    border-radius: 20px;
    padding: 2px 10px;
    font-size: 0.72rem;
    font-weight: 600;
  }
  .qt-progress-text { font-size: 0.78rem; color: rgba(255, 255, 255, 0.4); }

  .qt-progress-bar {
    width: 100%;
    height: 5px;
    background: rgba(255, 255, 255, 0.07);
    border-radius: 3px;
    overflow: hidden;
    margin-bottom: 24px;
  }
  .qt-progress-fill {
    height: 100%;
    background: linear-gradient(90deg, #60a5fa, #34d399);
    transition: width 0.2s ease;
  }

  .qt-questions-list { display: flex; flex-direction: column; gap: 20px; }
  .qt-question-block {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 12px;
    padding: 18px 20px;
  }
  .qt-q-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 10px; }
  .qt-q-number { font-size: 0.8rem; font-weight: 700; color: rgba(255, 255, 255, 0.5); }
  .qt-q-tags { display: flex; gap: 8px; align-items: center; }
  .qt-q-chapter {
    font-size: 0.68rem;
    font-family: monospace;
    color: rgba(255, 255, 255, 0.35);
  }
  .qt-q-difficulty {
    font-size: 0.68rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    border-radius: 4px;
    padding: 2px 7px;
  }
  .qt-q-difficulty.easy { background: rgba(52, 211, 153, 0.12); color: #34d399; }
  .qt-q-difficulty.medium { background: rgba(240, 192, 64, 0.12); color: #f0c040; }
  .qt-q-difficulty.hard { background: rgba(251, 113, 133, 0.12); color: #fb7185; }

  .qt-q-scenario { font-size: 0.88rem; line-height: 1.55; color: #dedeea; margin: 0 0 16px; }

  .qt-q-options { display: flex; flex-direction: column; gap: 8px; }
  .qt-option-row {
    display: flex;
    align-items: center;
    gap: 10px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    padding: 10px 14px;
    text-align: left;
    color: rgba(255, 255, 255, 0.75);
    font-size: 0.82rem;
    cursor: pointer;
    transition: all 0.12s;
  }
  .qt-option-row:hover { background: rgba(255, 255, 255, 0.06); }
  .qt-option-row.selected {
    background: rgba(96, 165, 250, 0.12);
    border-color: rgba(96, 165, 250, 0.45);
    color: #fff;
  }
  .qt-option-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    border: 1.5px solid rgba(255, 255, 255, 0.3);
    flex-shrink: 0;
  }
  .qt-option-row.selected .qt-option-dot {
    background: #60a5fa;
    border-color: #60a5fa;
  }

  .qt-submit-bar {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 16px;
    flex-wrap: wrap;
    margin-top: 24px;
    padding-top: 20px;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
  }
  .qt-submit-hint { font-size: 0.76rem; color: rgba(255, 255, 255, 0.35); }
  .qt-submit-btn {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    background: linear-gradient(135deg, #34d399, #60a5fa);
    border: none;
    color: #0c0e1a;
    font-weight: 700;
    border-radius: 10px;
    padding: 12px 24px;
    font-size: 0.9rem;
    cursor: pointer;
    transition: filter 0.15s, transform 0.15s;
  }
  .qt-submit-btn:hover:not(:disabled) { filter: brightness(1.08); transform: translateY(-1px); }
  .qt-submit-btn:disabled { opacity: 0.5; cursor: not-allowed; }

  /* ── Results ───────────────────────────────────────────────────────── */
  .qt-ai-flag-banner {
    background: rgba(251, 113, 133, 0.1);
    border: 1px solid rgba(251, 113, 133, 0.3);
    color: #fb7185;
    border-radius: 10px;
    padding: 12px 16px;
    font-size: 0.82rem;
    margin-bottom: 20px;
  }
  .qt-results-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
    gap: 1px;
    background: rgba(255, 255, 255, 0.07);
    border: 1px solid rgba(255, 255, 255, 0.09);
    border-radius: 14px;
    overflow: hidden;
    margin-bottom: 24px;
  }
  .qt-result-card {
    background: rgba(12, 14, 26, 0.55);
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .qt-result-label {
    font-size: 0.68rem;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.35);
  }
  .qt-result-value { font-size: 1.5rem; font-weight: 800; color: #f0f0ff; letter-spacing: -0.02em; }
  .qt-result-value.positive { color: #34d399; }
  .qt-result-value.negative { color: #fb7185; }
  .qt-result-sub { font-size: 0.74rem; color: rgba(255, 255, 255, 0.35); }

  /* ── Spinner ───────────────────────────────────────────────────────── */
  .spinner {
    display: inline-block;
    width: 12px;
    height: 12px;
    border: 2px solid rgba(12, 14, 26, 0.3);
    border-top-color: #0c0e1a;
    border-radius: 50%;
    animation: qt-spin 0.7s linear infinite;
    flex-shrink: 0;
  }
  @keyframes qt-spin { to { transform: rotate(360deg); } }

  /* ── Responsive ────────────────────────────────────────────────────── */
  @media (max-width: 680px) {
    .qt-root { padding: 18px 16px; }
    .qt-submit-bar { justify-content: stretch; }
    .qt-submit-btn { width: 100%; justify-content: center; }
  }
</style>
