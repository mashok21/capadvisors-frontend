<script>
  import { onMount } from 'svelte';
  import { fade, slide } from 'svelte/transition';
  import { auth } from '../auth.svelte.js';

  let { baseApiUrl = '' } = $props();

  // ── Core state ────────────────────────────────────────────────────────────
  let pendingQuestions = $state([]);
  let selectedQuestion = $state(null);
  let guidanceText     = $state('');
  let isProcessing     = $state(null); // null | 'load'|'save'|'improvise'|'refine'|'approve'|'reject'

  // Inline-edit mirrors — synced from selectedQuestion on selection
  let editQuestionText  = $state('');
  let editCorrectAnswer = $state('');

  // Flash messages
  let flashMsg   = $state('');
  let flashKind  = $state(''); // 'ok' | 'err'
  let flashTimer = null;

  // Expandable variant cards
  let openVariant = $state(null); // variant_number | null

  // ── Derived ───────────────────────────────────────────────────────────────
  let parsedRubric = $derived.by(() => {
    if (!selectedQuestion?.scoring_rubric_json) return null;
    try { return JSON.parse(selectedQuestion.scoring_rubric_json); }
    catch { return null; }
  });

  let marksSum = $derived(
    parsedRubric?.steps?.reduce((a, s) => a + (s.marks ?? 0), 0) ?? 0
  );

  let marksOk = $derived(
    parsedRubric
      ? Math.abs(marksSum - (parsedRubric.total_marks ?? 0)) <= 0.5
      : true
  );

  let parsedVariants = $derived.by(() => {
    if (!selectedQuestion?.alternate_variants_json) return [];
    try { return JSON.parse(selectedQuestion.alternate_variants_json); }
    catch { return []; }
  });

  let isDirty = $derived(
    !!selectedQuestion && (
      editQuestionText  !== selectedQuestion.question_text ||
      editCorrectAnswer !== (selectedQuestion.correct_answer ?? '')
    )
  );

  let busy = $derived(isProcessing !== null);

  // ── Helpers ───────────────────────────────────────────────────────────────
  function authHeaders() {
    return {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${auth.token}`,
    };
  }

  function flash(msg, kind = 'ok') {
    flashMsg  = msg;
    flashKind = kind;
    clearTimeout(flashTimer);
    flashTimer = setTimeout(() => { flashMsg = ''; flashKind = ''; }, 4500);
  }

  function applyUpdate(updated) {
    selectedQuestion  = updated;
    editQuestionText  = updated.question_text;
    editCorrectAnswer = updated.correct_answer ?? '';
    pendingQuestions  = pendingQuestions.map(q => q.id === updated.id ? updated : q);
  }

  function removeFromQueue(id) {
    pendingQuestions = pendingQuestions.filter(q => q.id !== id);
    const next = pendingQuestions[0] ?? null;
    if (next) selectQuestion(next);
    else {
      selectedQuestion  = null;
      editQuestionText  = '';
      editCorrectAnswer = '';
    }
  }

  function selectQuestion(q) {
    selectedQuestion  = q;
    editQuestionText  = q.question_text;
    editCorrectAnswer = q.correct_answer ?? '';
    guidanceText      = '';
    openVariant       = null;
    flashMsg          = '';
  }

  function stepBarWidth(marks, total) {
    if (!total) return 2;
    return Math.max(2, (marks / total) * 100);
  }

  // ── API calls ─────────────────────────────────────────────────────────────
  async function loadQueue() {
    isProcessing = 'load';
    try {
      const res = await fetch(`${baseApiUrl}/api/admin/questions/staging`, {
        headers: authHeaders(),
      });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      pendingQuestions = await res.json();
      if (pendingQuestions.length && !selectedQuestion) selectQuestion(pendingQuestions[0]);
    } catch (e) {
      flash(`Queue load failed: ${e.message}`, 'err');
    } finally {
      isProcessing = null;
    }
  }

  async function handleSave() {
    if (!selectedQuestion || !isDirty) return;
    isProcessing = 'save';
    try {
      const res = await fetch(`${baseApiUrl}/api/admin/questions/staging/${selectedQuestion.id}`, {
        method: 'PUT',
        headers: authHeaders(),
        body: JSON.stringify({
          question_text:           editQuestionText,
          scoring_rubric_json:     selectedQuestion.scoring_rubric_json,
          alternate_variants_json: selectedQuestion.alternate_variants_json,
          correct_answer:          editCorrectAnswer,
        }),
      });
      if (!res.ok) throw new Error(await res.text());
      applyUpdate(await res.json());
      flash('Edits saved successfully.');
    } catch (e) {
      flash(`Save failed: ${e.message}`, 'err');
    } finally {
      isProcessing = null;
    }
  }

  async function handleImprovise() {
    if (!selectedQuestion) return;
    isProcessing = 'improvise';
    try {
      const res = await fetch(`${baseApiUrl}/api/admin/questions/staging/${selectedQuestion.id}/improvise`, {
        method: 'POST',
        headers: authHeaders(),
        body: JSON.stringify({ admin_guidance: guidanceText.trim() || null }),
      });
      if (!res.ok) throw new Error(await res.text());
      applyUpdate(await res.json());
      flash('Question improvised — all three sections updated.');
    } catch (e) {
      flash(`Improvise failed: ${e.message}`, 'err');
    } finally {
      isProcessing = null;
    }
  }

  async function handleRefine() {
    if (!selectedQuestion) return;
    isProcessing = 'refine';
    try {
      const res = await fetch(`${baseApiUrl}/api/admin/questions/staging/${selectedQuestion.id}/refine-answer`, {
        method: 'POST',
        headers: authHeaders(),
        body: JSON.stringify({ admin_guidance: guidanceText.trim() || null }),
      });
      if (!res.ok) throw new Error(await res.text());
      applyUpdate(await res.json());
      flash('Scoring rubric refined — narrative preserved.');
    } catch (e) {
      flash(`Refine failed: ${e.message}`, 'err');
    } finally {
      isProcessing = null;
    }
  }

  async function handleApprove() {
    if (!selectedQuestion) return;
    isProcessing = 'approve';
    try {
      const res = await fetch(`${baseApiUrl}/api/admin/questions/staging/${selectedQuestion.id}/approve`, {
        method: 'POST',
        headers: authHeaders(),
      });
      if (!res.ok) throw new Error(await res.text());
      removeFromQueue(selectedQuestion.id);
      flash('Question approved and promoted to live databank.');
    } catch (e) {
      flash(`Approve failed: ${e.message}`, 'err');
    } finally {
      isProcessing = null;
    }
  }

  async function handleReject() {
    if (!selectedQuestion) return;
    isProcessing = 'reject';
    try {
      const res = await fetch(`${baseApiUrl}/api/admin/questions/staging/${selectedQuestion.id}/reject`, {
        method: 'DELETE',
        headers: authHeaders(),
      });
      if (!res.ok && res.status !== 204) throw new Error(await res.text());
      removeFromQueue(selectedQuestion.id);
      flash('Question rejected and removed from queue.');
    } catch (e) {
      flash(`Reject failed: ${e.message}`, 'err');
    } finally {
      isProcessing = null;
    }
  }

  onMount(loadQueue);
</script>

<!-- ── Root ───────────────────────────────────────────────────────────────── -->
<div class="ar-root">

  <!-- Top bar -->
  <div class="ar-topbar">
    <div class="ar-title">
      <span class="ar-icon">⚙</span>
      Admin Review Panel
      {#if pendingQuestions.length > 0}
        <span class="queue-badge">{pendingQuestions.length} pending</span>
      {:else if isProcessing !== 'load'}
        <span class="queue-badge empty">Queue clear</span>
      {/if}
    </div>
    <button class="topbar-refresh" onclick={loadQueue} disabled={busy}>
      {#if isProcessing === 'load'}<span class="spin-sm"></span>{:else}🔄{/if}
      Refresh
    </button>
  </div>

  <!-- Flash message -->
  {#if flashMsg}
    <div class="flash {flashKind === 'err' ? 'flash-err' : 'flash-ok'}" transition:fade={{ duration: 200 }}>
      {#if flashKind === 'err'}⚠{:else}✓{/if}
      {flashMsg}
    </div>
  {/if}

  <!-- Main split layout -->
  <div class="ar-body">

    <!-- ── Left Panel (45%) ────────────────────────────────────────────── -->
    <div class="left-panel">

      <!-- Queue list -->
      <div class="queue-section">
        <div class="queue-section-label">
          Staging Queue
          {#if isProcessing === 'load'}<span class="spin-sm"></span>{/if}
        </div>
        {#if pendingQuestions.length === 0 && isProcessing !== 'load'}
          <div class="queue-empty">No questions pending review</div>
        {:else}
          <div class="queue-list">
            {#each pendingQuestions as q (q.id)}
              <button
                class="queue-item {selectedQuestion?.id === q.id ? 'qi-active' : ''}"
                onclick={() => selectQuestion(q)}
                disabled={busy && isProcessing !== 'load'}
              >
                <span class="qi-dot"></span>
                <span class="qi-text">{q.question_text.slice(0, 72)}{q.question_text.length > 72 ? '…' : ''}</span>
              </button>
            {/each}
          </div>
        {/if}
      </div>

      <!-- Edit form — only when a question is selected -->
      {#if selectedQuestion}
        <div class="edit-form" transition:fade={{ duration: 150 }}>

          <div class="field-group">
            <label class="field-label">
              Question Narrative
              {#if isDirty}<span class="dirty-pill">unsaved</span>{/if}
            </label>
            <textarea
              class="field-textarea"
              bind:value={editQuestionText}
              rows="7"
              disabled={busy}
            ></textarea>
          </div>

          <div class="field-group">
            <label class="field-label">Answer Key</label>
            <input
              class="field-input"
              bind:value={editCorrectAnswer}
              placeholder="e.g. Option B: INR 684,000"
              disabled={busy}
            />
          </div>

          <div class="field-group">
            <label class="field-label ai-label">
              <span class="ai-star">✦</span>
              AI Guidance
              <span class="field-hint">Applied to Improvise &amp; Refine operations below</span>
            </label>
            <textarea
              class="field-textarea guidance-textarea"
              bind:value={guidanceText}
              rows="3"
              placeholder="e.g. Format NPV steps in LaTeX notation. Verify the rounding on option premium calculations."
              disabled={busy}
            ></textarea>
          </div>

          <!-- Action button rows -->
          <div class="action-strip">

            <button
              class="btn btn-save"
              onclick={handleSave}
              disabled={!isDirty || busy}
              title="Persist manual edits to question text and answer key"
            >
              {#if isProcessing === 'save'}<span class="spin-sm"></span>{/if}
              Save Edits
            </button>

            <div class="action-divider"><span>AI Operations</span></div>

            <div class="ai-action-row">
              <button
                class="btn btn-improvise"
                onclick={handleImprovise}
                disabled={busy}
                title="Rewrite full question, rubric, and variants via Gemini (temp 0.2)"
              >
                {#if isProcessing === 'improvise'}
                  <span class="spin-sm"></span> Improvising…
                {:else}
                  ✦ Improvise
                {/if}
              </button>

              <button
                class="btn btn-refine"
                onclick={handleRefine}
                disabled={busy}
                title="Refine scoring rubric only — narrative preserved (temp 0.1)"
              >
                {#if isProcessing === 'refine'}
                  <span class="spin-sm"></span> Refining…
                {:else}
                  ◎ Refine Answer
                {/if}
              </button>
            </div>

            <div class="action-divider"><span>Verdict</span></div>

            <div class="verdict-row">
              <button
                class="btn btn-approve"
                onclick={handleApprove}
                disabled={busy}
                title="Promote to live quiz_databank with Glicko-2 seed values"
              >
                {#if isProcessing === 'approve'}
                  <span class="spin-sm"></span> Approving…
                {:else}
                  ✓ Approve &amp; Publish
                {/if}
              </button>

              <button
                class="btn btn-reject"
                onclick={handleReject}
                disabled={busy}
                title="Soft-delete — status set to rejected, preserved for audit"
              >
                {#if isProcessing === 'reject'}
                  <span class="spin-sm"></span>
                {:else}
                  ✕
                {/if}
                Reject
              </button>
            </div>

          </div>
        </div>

      {:else}
        <div class="form-placeholder">
          <span class="placeholder-icon">📋</span>
          <span>Select a question from the queue to begin review</span>
        </div>
      {/if}
    </div>

    <!-- ── Right Panel (55%) — Live Preview ───────────────────────────── -->
    <div class="right-panel">
      {#if !selectedQuestion}
        <div class="preview-empty">
          <span class="pe-icon">📝</span>
          <p>No question selected</p>
          <p class="pe-sub">Pick a question from the queue on the left to see the formatted preview</p>
        </div>
      {:else}
        <div class="preview-scroll">

          <!-- ── Case Scenario ─────────────────────────────────────────── -->
          <section class="preview-section">
            <div class="pv-header">
              <span class="pv-label">Case Scenario</span>
              <span class="pv-meta">
                {selectedQuestion.chapter_id
                  ? selectedQuestion.chapter_id.slice(0, 8) + '…'
                  : '—'}
              </span>
            </div>
            <div class="pv-question-text">{selectedQuestion.question_text}</div>
          </section>

          <!-- ── Scoring Rubric ────────────────────────────────────────── -->
          <section class="preview-section">
            <div class="pv-header">
              <span class="pv-label">Scoring Rubric</span>
              {#if parsedRubric}
                <span class="marks-badge {marksOk ? 'mb-ok' : 'mb-warn'}">
                  {marksSum.toFixed(1)} / {parsedRubric.total_marks} marks
                  {marksOk ? '✓' : '⚠'}
                </span>
              {/if}
            </div>

            {#if !parsedRubric}
              <div class="rubric-invalid">
                ⚠ Could not parse scoring_rubric_json — run Improvise or Refine to regenerate.
              </div>
            {:else}
              <!-- Step table -->
              <div class="rubric-table">
                <div class="rt-head">
                  <span class="rtc-num">#</span>
                  <span class="rtc-desc">Description</span>
                  <span class="rtc-marks">Marks</span>
                </div>
                {#each parsedRubric.steps as step (step.step)}
                  <div class="rt-row">
                    <span class="rtc-num rt-step-num">{step.step}</span>
                    <span class="rtc-desc rt-step-desc">{step.description}</span>
                    <span class="rtc-marks rt-step-marks">{step.marks}</span>
                  </div>
                {/each}
              </div>

              <!-- Marks distribution bar -->
              <div class="marks-dist-bar" title="Mark allocation across steps">
                {#each parsedRubric.steps as step, i}
                  <div
                    class="mdb-seg"
                    style="width: {stepBarWidth(step.marks, parsedRubric.total_marks)}%; opacity: {0.45 + (step.marks / (parsedRubric.total_marks || 1)) * 0.55}"
                    title="Step {step.step}: {step.marks}m"
                  ></div>
                {/each}
              </div>

              <!-- Marks summary -->
              <div class="marks-summary {marksOk ? '' : 'marks-summary-warn'}">
                <span class="ms-computed">
                  Computed sum: <strong>{marksSum.toFixed(2)}</strong>
                </span>
                <span class="ms-sep">·</span>
                <span class="ms-total">
                  Target: <strong>{parsedRubric.total_marks}</strong>
                </span>
                {#if !marksOk}
                  <span class="ms-delta">
                    Δ {Math.abs(marksSum - parsedRubric.total_marks).toFixed(2)} exceeds ±0.5 tolerance — run Refine Answer
                  </span>
                {/if}
              </div>
            {/if}
          </section>

          <!-- ── Answer Key ────────────────────────────────────────────── -->
          {#if selectedQuestion.correct_answer}
            <section class="preview-section">
              <div class="pv-header">
                <span class="pv-label">Answer Key</span>
              </div>
              <div class="pv-answer-key">{selectedQuestion.correct_answer}</div>
            </section>
          {:else}
            <section class="preview-section">
              <div class="pv-header">
                <span class="pv-label">Answer Key</span>
                <span class="pv-warn-tag">Not set — required before approval</span>
              </div>
            </section>
          {/if}

          <!-- ── Diagnostic Variants ───────────────────────────────────── -->
          {#if parsedVariants.length > 0}
            <section class="preview-section">
              <div class="pv-header">
                <span class="pv-label">Diagnostic Variants</span>
                <span class="pv-meta">{parsedVariants.length} variant{parsedVariants.length !== 1 ? 's' : ''}</span>
              </div>
              <div class="variants-list">
                {#each parsedVariants as v (v.variant_number)}
                  <div class="variant-card">
                    <button
                      class="variant-toggle {openVariant === v.variant_number ? 'vt-open' : ''}"
                      onclick={() => openVariant = openVariant === v.variant_number ? null : v.variant_number}
                    >
                      <span class="vt-num">Variant {v.variant_number}</span>
                      <span class="vt-type">{v.variant_type}</span>
                      <span class="vt-title">{v.title}</span>
                      <span class="vt-caret">{openVariant === v.variant_number ? '▲' : '▼'}</span>
                    </button>
                    {#if openVariant === v.variant_number}
                      <div class="variant-body" transition:slide={{ duration: 180 }}>
                        <p class="vb-question">{v.question}</p>
                        <div class="vb-diff">
                          <span class="vb-diff-label">Δ Key difference</span>
                          {v.key_difference}
                        </div>
                      </div>
                    {/if}
                  </div>
                {/each}
              </div>
            </section>
          {/if}

        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  /* ── Root ────────────────────────────────────────────────────────────── */
  .ar-root {
    width: 100%;
    background: rgba(255, 255, 255, 0.018);
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 16px;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  /* ── Topbar ──────────────────────────────────────────────────────────── */
  .ar-topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 22px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.07);
    background: rgba(255, 255, 255, 0.025);
    flex-shrink: 0;
  }
  .ar-title {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 0.95rem;
    font-weight: 700;
    color: #f0f0ff;
    letter-spacing: -0.01em;
  }
  .ar-icon { font-size: 1rem; opacity: 0.7; }
  .queue-badge {
    font-size: 0.7rem;
    font-weight: 700;
    background: rgba(240, 192, 64, 0.15);
    border: 1px solid rgba(240, 192, 64, 0.3);
    color: #f0c040;
    border-radius: 20px;
    padding: 2px 9px;
  }
  .queue-badge.empty {
    background: rgba(52, 211, 153, 0.1);
    border-color: rgba(52, 211, 153, 0.25);
    color: #34d399;
  }
  .topbar-refresh {
    display: flex;
    align-items: center;
    gap: 6px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.09);
    color: rgba(255, 255, 255, 0.6);
    border-radius: 8px;
    padding: 6px 13px;
    font-size: 0.78rem;
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
  }
  .topbar-refresh:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.09);
    color: #fff;
  }
  .topbar-refresh:disabled { opacity: 0.4; cursor: not-allowed; }

  /* ── Flash messages ──────────────────────────────────────────────────── */
  .flash {
    padding: 10px 22px;
    font-size: 0.82rem;
    font-weight: 500;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .flash-ok  { background: rgba(52, 211, 153, 0.1);  color: #34d399;  border-bottom: 1px solid rgba(52, 211, 153, 0.2); }
  .flash-err { background: rgba(251, 113, 133, 0.1); color: #fb7185; border-bottom: 1px solid rgba(251, 113, 133, 0.2); }

  /* ── Main body ───────────────────────────────────────────────────────── */
  .ar-body {
    display: flex;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  /* ── Left Panel ──────────────────────────────────────────────────────── */
  .left-panel {
    width: 45%;
    min-width: 260px;
    border-right: 1px solid rgba(255, 255, 255, 0.07);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  /* Queue section */
  .queue-section {
    flex-shrink: 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    padding: 12px 0 0;
  }
  .queue-section-label {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 16px 8px;
    font-size: 0.68rem;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.3);
  }
  .queue-empty {
    padding: 14px 16px;
    font-size: 0.8rem;
    color: rgba(255, 255, 255, 0.3);
    font-style: italic;
  }
  .queue-list {
    max-height: 200px;
    overflow-y: auto;
  }
  .queue-list::-webkit-scrollbar { width: 4px; }
  .queue-list::-webkit-scrollbar-track { background: transparent; }
  .queue-list::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.1); border-radius: 2px; }

  .queue-item {
    width: 100%;
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 10px 16px;
    background: transparent;
    border: none;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
    cursor: pointer;
    text-align: left;
    transition: background 0.12s;
  }
  .queue-item:hover { background: rgba(255, 255, 255, 0.03); }
  .queue-item.qi-active { background: rgba(96, 165, 250, 0.07); }
  .queue-item:disabled { opacity: 0.5; cursor: not-allowed; }
  .qi-dot {
    flex-shrink: 0;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: rgba(240, 192, 64, 0.6);
    margin-top: 6px;
  }
  .qi-active .qi-dot { background: #60a5fa; }
  .qi-text {
    font-size: 0.76rem;
    color: rgba(255, 255, 255, 0.6);
    line-height: 1.45;
  }
  .qi-active .qi-text { color: rgba(255, 255, 255, 0.85); }

  /* Edit form */
  .edit-form {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .edit-form::-webkit-scrollbar { width: 4px; }
  .edit-form::-webkit-scrollbar-track { background: transparent; }
  .edit-form::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.1); border-radius: 2px; }

  .form-placeholder {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    color: rgba(255, 255, 255, 0.25);
    font-size: 0.82rem;
    padding: 32px 16px;
  }
  .placeholder-icon { font-size: 2rem; opacity: 0.4; }

  /* Form fields */
  .field-group { display: flex; flex-direction: column; gap: 5px; }
  .field-label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.7rem;
    font-weight: 600;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.38);
  }
  .ai-label { color: rgba(167, 139, 250, 0.8); }
  .ai-star { color: #a78bfa; font-size: 0.75rem; }
  .field-hint {
    font-size: 0.62rem;
    color: rgba(255, 255, 255, 0.25);
    text-transform: none;
    letter-spacing: 0;
    font-weight: 400;
  }
  .dirty-pill {
    font-size: 0.6rem;
    background: rgba(240, 192, 64, 0.15);
    color: #f0c040;
    border-radius: 4px;
    padding: 1px 5px;
    text-transform: none;
    letter-spacing: 0;
    font-weight: 600;
  }
  .field-textarea, .field-input {
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.09);
    border-radius: 8px;
    color: rgba(255, 255, 255, 0.85);
    font-size: 0.8rem;
    font-family: inherit;
    line-height: 1.55;
    padding: 9px 12px;
    resize: vertical;
    transition: border-color 0.15s;
    width: 100%;
  }
  .field-textarea:focus, .field-input:focus {
    outline: none;
    border-color: rgba(96, 165, 250, 0.4);
    background: rgba(255, 255, 255, 0.06);
  }
  .field-textarea:disabled, .field-input:disabled { opacity: 0.45; cursor: not-allowed; }
  .guidance-textarea { font-size: 0.77rem; color: rgba(255, 255, 255, 0.75); }

  /* Action strip */
  .action-strip { display: flex; flex-direction: column; gap: 8px; margin-top: 4px; }
  .action-divider {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 4px 0 2px;
  }
  .action-divider::before, .action-divider::after {
    content: '';
    flex: 1;
    height: 1px;
    background: rgba(255, 255, 255, 0.07);
  }
  .action-divider span {
    font-size: 0.62rem;
    font-weight: 600;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.22);
    white-space: nowrap;
  }
  .ai-action-row, .verdict-row {
    display: flex;
    gap: 8px;
  }
  .ai-action-row .btn, .verdict-row .btn { flex: 1; }

  /* Buttons */
  .btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    border-radius: 8px;
    font-size: 0.78rem;
    font-weight: 600;
    padding: 8px 14px;
    cursor: pointer;
    border: 1px solid transparent;
    transition: background 0.15s, opacity 0.15s, transform 0.1s;
    white-space: nowrap;
  }
  .btn:disabled { opacity: 0.38; cursor: not-allowed; transform: none !important; }
  .btn:hover:not(:disabled) { transform: translateY(-1px); }
  .btn:active:not(:disabled) { transform: translateY(0); }

  .btn-save {
    background: rgba(255, 255, 255, 0.06);
    border-color: rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.7);
  }
  .btn-save:hover:not(:disabled) { background: rgba(255,255,255,0.1); color: #fff; }

  .btn-improvise {
    background: rgba(167, 139, 250, 0.12);
    border-color: rgba(167, 139, 250, 0.3);
    color: #a78bfa;
  }
  .btn-improvise:hover:not(:disabled) { background: rgba(167,139,250,0.2); }

  .btn-refine {
    background: rgba(96, 165, 250, 0.12);
    border-color: rgba(96, 165, 250, 0.3);
    color: #60a5fa;
  }
  .btn-refine:hover:not(:disabled) { background: rgba(96,165,250,0.2); }

  .btn-approve {
    background: rgba(52, 211, 153, 0.12);
    border-color: rgba(52, 211, 153, 0.3);
    color: #34d399;
    font-weight: 700;
  }
  .btn-approve:hover:not(:disabled) { background: rgba(52,211,153,0.2); }

  .btn-reject {
    background: rgba(251, 113, 133, 0.09);
    border-color: rgba(251, 113, 133, 0.25);
    color: #fb7185;
    flex: 0 0 auto;
    min-width: 80px;
  }
  .btn-reject:hover:not(:disabled) { background: rgba(251,113,133,0.18); }

  /* ── Right Panel ─────────────────────────────────────────────────────── */
  .right-panel {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .preview-empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    color: rgba(255, 255, 255, 0.25);
    padding: 48px 24px;
  }
  .pe-icon { font-size: 2.5rem; opacity: 0.35; }
  .preview-empty p { font-size: 0.88rem; }
  .pe-sub { font-size: 0.75rem; color: rgba(255,255,255,0.18); text-align: center; }

  .preview-scroll {
    flex: 1;
    overflow-y: auto;
    padding: 20px 22px 28px;
    display: flex;
    flex-direction: column;
    gap: 0;
  }
  .preview-scroll::-webkit-scrollbar { width: 5px; }
  .preview-scroll::-webkit-scrollbar-track { background: transparent; }
  .preview-scroll::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.1); border-radius: 2px; }

  /* Preview sections */
  .preview-section {
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    padding: 18px 0;
  }
  .preview-section:last-child { border-bottom: none; }

  .pv-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 12px;
    flex-wrap: wrap;
  }
  .pv-label {
    font-size: 0.68rem;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.32);
  }
  .pv-meta {
    font-size: 0.68rem;
    color: rgba(255, 255, 255, 0.25);
    font-family: monospace;
  }
  .pv-warn-tag {
    font-size: 0.68rem;
    background: rgba(251, 113, 133, 0.12);
    color: #fb7185;
    border: 1px solid rgba(251, 113, 133, 0.25);
    border-radius: 4px;
    padding: 2px 7px;
    font-weight: 600;
  }

  .pv-question-text {
    font-size: 0.85rem;
    line-height: 1.7;
    color: rgba(255, 255, 255, 0.82);
    white-space: pre-wrap;
  }

  /* Marks badge */
  .marks-badge {
    font-size: 0.72rem;
    font-weight: 700;
    border-radius: 6px;
    padding: 3px 9px;
  }
  .mb-ok  { background: rgba(52, 211, 153, 0.12); color: #34d399; border: 1px solid rgba(52,211,153,0.25); }
  .mb-warn { background: rgba(251, 113, 133, 0.12); color: #fb7185; border: 1px solid rgba(251,113,133,0.25); }

  /* Rubric table */
  .rubric-invalid {
    font-size: 0.8rem;
    color: #fb7185;
    padding: 10px 12px;
    background: rgba(251,113,133,0.07);
    border-radius: 8px;
    border: 1px solid rgba(251,113,133,0.2);
  }
  .rubric-table {
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 10px;
    overflow: hidden;
    font-size: 0.8rem;
  }
  .rt-head {
    display: grid;
    grid-template-columns: 36px 1fr 60px;
    gap: 0;
    padding: 8px 12px;
    background: rgba(255, 255, 255, 0.04);
    border-bottom: 1px solid rgba(255, 255, 255, 0.07);
    font-size: 0.66rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.28);
  }
  .rt-row {
    display: grid;
    grid-template-columns: 36px 1fr 60px;
    gap: 0;
    padding: 9px 12px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
    align-items: start;
    transition: background 0.1s;
  }
  .rt-row:last-child { border-bottom: none; }
  .rt-row:hover { background: rgba(255,255,255,0.025); }
  .rtc-num { font-variant-numeric: tabular-nums; }
  .rtc-marks { text-align: right; }
  .rt-step-num {
    font-size: 0.72rem;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.35);
    padding-top: 1px;
  }
  .rt-step-desc {
    font-size: 0.8rem;
    line-height: 1.5;
    color: rgba(255, 255, 255, 0.78);
    padding-right: 8px;
  }
  .rt-step-marks {
    font-size: 0.82rem;
    font-weight: 700;
    color: #60a5fa;
    font-variant-numeric: tabular-nums;
  }

  /* Marks distribution bar */
  .marks-dist-bar {
    display: flex;
    gap: 2px;
    margin-top: 10px;
    height: 5px;
    border-radius: 3px;
    overflow: hidden;
  }
  .mdb-seg {
    background: linear-gradient(90deg, #60a5fa, #a78bfa);
    border-radius: 1px;
    transition: opacity 0.2s;
  }

  /* Marks summary */
  .marks-summary {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    margin-top: 8px;
    padding: 7px 10px;
    border-radius: 7px;
    background: rgba(52, 211, 153, 0.06);
    border: 1px solid rgba(52, 211, 153, 0.12);
    font-size: 0.76rem;
    color: rgba(255, 255, 255, 0.55);
  }
  .marks-summary-warn {
    background: rgba(251, 113, 133, 0.06);
    border-color: rgba(251, 113, 133, 0.18);
  }
  .ms-computed strong, .ms-total strong { color: rgba(255,255,255,0.85); }
  .ms-sep { color: rgba(255,255,255,0.2); }
  .ms-delta {
    font-size: 0.72rem;
    color: #fb7185;
    font-weight: 600;
    width: 100%;
  }

  /* Answer key */
  .pv-answer-key {
    font-size: 0.85rem;
    font-weight: 600;
    color: #34d399;
    padding: 9px 12px;
    background: rgba(52, 211, 153, 0.07);
    border: 1px solid rgba(52, 211, 153, 0.18);
    border-radius: 8px;
  }

  /* Diagnostic variants */
  .variants-list { display: flex; flex-direction: column; gap: 6px; }
  .variant-card {
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 9px;
    overflow: hidden;
  }
  .variant-toggle {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 14px;
    background: rgba(255, 255, 255, 0.025);
    border: none;
    cursor: pointer;
    text-align: left;
    transition: background 0.12s;
  }
  .variant-toggle:hover { background: rgba(255,255,255,0.04); }
  .variant-toggle.vt-open { background: rgba(96, 165, 250, 0.06); }
  .vt-num {
    font-size: 0.68rem;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.35);
    white-space: nowrap;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .vt-type {
    font-size: 0.62rem;
    font-weight: 700;
    font-family: monospace;
    background: rgba(167, 139, 250, 0.12);
    color: #a78bfa;
    border-radius: 4px;
    padding: 1px 5px;
    white-space: nowrap;
  }
  .vt-title {
    flex: 1;
    font-size: 0.78rem;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.75);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .vt-caret { font-size: 0.6rem; color: rgba(255,255,255,0.25); flex-shrink: 0; }
  .variant-body {
    padding: 12px 14px 14px;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
    background: rgba(255, 255, 255, 0.012);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .vb-question {
    font-size: 0.8rem;
    line-height: 1.6;
    color: rgba(255, 255, 255, 0.75);
    white-space: pre-wrap;
  }
  .vb-diff {
    font-size: 0.76rem;
    color: rgba(255, 255, 255, 0.5);
    padding: 8px 10px;
    background: rgba(255, 255, 255, 0.03);
    border-left: 2px solid rgba(96, 165, 250, 0.35);
    border-radius: 0 6px 6px 0;
  }
  .vb-diff-label {
    display: block;
    font-size: 0.62rem;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: rgba(96, 165, 250, 0.6);
    margin-bottom: 4px;
  }

  /* Spinner */
  .spin-sm {
    display: inline-block;
    width: 11px;
    height: 11px;
    border: 2px solid rgba(255, 255, 255, 0.2);
    border-top-color: currentColor;
    border-radius: 50%;
    animation: ar-spin 0.7s linear infinite;
    flex-shrink: 0;
  }
  @keyframes ar-spin { to { transform: rotate(360deg); } }

  /* ── Responsive ──────────────────────────────────────────────────────── */
  @media (max-width: 860px) {
    .ar-body { flex-direction: column; overflow: auto; }
    .left-panel {
      width: 100%;
      border-right: none;
      border-bottom: 1px solid rgba(255, 255, 255, 0.07);
      max-height: 60vh;
    }
    .right-panel { max-height: none; }
    .queue-list { max-height: 130px; }
  }

  @media (max-width: 540px) {
    .ai-action-row, .verdict-row { flex-direction: column; }
    .btn-reject { min-width: unset; }
    .ar-topbar { flex-wrap: wrap; gap: 8px; }
  }
</style>
