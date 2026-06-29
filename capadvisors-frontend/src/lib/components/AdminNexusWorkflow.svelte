<script>
  import { auth } from '../auth.svelte.js';
  import AdminReviewPanel from './AdminReviewPanel.svelte';

  let { baseApiUrl = '' } = $props();

  const PHASES = [
    { num: 1, label: 'Chapter Select' },
    { num: 2, label: 'File Ingestion' },
    { num: 3, label: 'Quiz Proofing' },
    { num: 4, label: 'Hackathon' },
    { num: 5, label: 'Archive' },
  ];

  const CHAPTERS = [
    { id: 'ch01', chapter_code: 'AFM-CH01', chapter_name: 'Financial Policy and Corporate Strategy' },
    { id: 'ch02', chapter_code: 'AFM-CH02', chapter_name: 'Risk Management' },
    { id: 'ch03', chapter_code: 'AFM-CH03', chapter_name: 'Advanced Capital Budgeting Decisions' },
    { id: 'ch04', chapter_code: 'AFM-CH04', chapter_name: 'Security Analysis' },
    { id: 'ch05', chapter_code: 'AFM-CH05', chapter_name: 'Security Valuation' },
    { id: 'ch06', chapter_code: 'AFM-CH06', chapter_name: 'Portfolio Management' },
    { id: 'ch07', chapter_code: 'AFM-CH07', chapter_name: 'Securitization' },
    { id: 'ch08', chapter_code: 'AFM-CH08', chapter_name: 'Mutual Funds' },
    { id: 'ch09', chapter_code: 'AFM-CH09', chapter_name: 'Derivatives Analysis and Valuation' },
    { id: 'ch10', chapter_code: 'AFM-CH10', chapter_name: 'Foreign Exchange Exposure and Risk Management' },
    { id: 'ch11', chapter_code: 'AFM-CH11', chapter_name: 'International Financial Management' },
    { id: 'ch12', chapter_code: 'AFM-CH12', chapter_name: 'Interest Rate Risk Management' },
    { id: 'ch13', chapter_code: 'AFM-CH13', chapter_name: 'Business Valuation' },
    { id: 'ch14', chapter_code: 'AFM-CH14', chapter_name: 'Mergers, Acquisitions and Corporate Restructuring' },
    { id: 'ch15', chapter_code: 'AFM-CH15', chapter_name: 'Startup Finance' },
  ];

  // ── Phase state ────────────────────────────────────────────────────────────
  let phase = $state(1);

  // Phase 1
  let selectedChapter = $state(null);

  // Phase 2
  let selectedFile    = $state(null);
  let dragOver        = $state(false);
  let isUploading     = $state(false);
  let uploadError     = $state('');
  let uploadSuccess   = $state(null);
  let dropzoneKey     = $state(0); // incremented on X-click to force full DOM teardown
  let isAnalyzing     = $state(false);
  let mappingStatus   = $state('idle'); // 'idle' | 'loading' | 'success' | 'error'

  function clearFile() {
    selectedFile  = null;
    uploadError   = '';
    uploadSuccess = null;
    mappingStatus = 'idle';
    dropzoneKey++;
  }

  function handleRemoveFile() {
    selectedFile  = null;
    uploadError   = '';
    mappingStatus = 'idle';
    dropzoneKey++;
  }

  // View masking (super admin impersonation preview)
  let currentViewMode = $state('super_admin'); // 'super_admin' | 'admin' | 'user'

  // Phase 3 — question browser
  let generatedQuestions    = $state([]);
  let currentQuestionIndex  = $state(0);
  let revealExplanation     = $state(false);
  let isFetchingQuestions   = $state(false);

  async function loadChapterQuestions(chapterId) {
    if (!chapterId) return;
    isFetchingQuestions = true;
    try {
      const res = await fetch(`${baseApiUrl}/api/nexus/chapters/${chapterId}/questions`);
      if (res.ok) {
        generatedQuestions   = await res.json();
        currentQuestionIndex = 0;
        revealExplanation    = false;
      }
    } catch (err) {
      console.error('Failed to load quiz sequence:', err);
    } finally {
      isFetchingQuestions = false;
    }
  }

  $effect(() => {
    if (phase === 3 && selectedChapter) {
      loadChapterQuestions(selectedChapter.id);
    }
  });

  // Phase 4
  let hackTitle    = $state('');
  let hackDate     = $state('');
  let hackDuration = $state('60');
  let hackSaved    = $state(false);

  // ── Derived ────────────────────────────────────────────────────────────────
  let canLeavePhase2 = $derived(uploadSuccess !== null);

  // ── Helpers ────────────────────────────────────────────────────────────────
  function handleFileSelect(e) {
    const files = e.target?.files ?? e.dataTransfer?.files;
    if (!files?.length) return;
    const file = files[0];
    const ext = file.name.split('.').pop().toLowerCase();
    if (ext === 'pdf' || ext === 'txt') {
      selectedFile = file;
      uploadError  = '';
    } else {
      uploadError = 'Invalid file type. Please upload a .pdf or .txt document.';
    }
  }

  async function triggerUpload() {
    if (!selectedFile || !selectedChapter) return;
    isUploading   = true;
    isAnalyzing   = true;
    mappingStatus = 'loading';
    uploadError   = '';

    const fd = new FormData();
    fd.append('file', selectedFile);
    fd.append('upload_type', 'TARGETED');
    fd.append('chapter_id', selectedChapter.id);

    try {
      const res = await fetch(`${baseApiUrl}/api/nexus/upload`, {
        method: 'POST',
        body: fd,
      });
      if (res.ok) {
        uploadSuccess = await res.json();
        selectedFile  = null;
        isAnalyzing   = false;
        mappingStatus = 'success';
      } else {
        uploadError   = (await res.text()) || `Upload failed (${res.status})`;
        isAnalyzing   = false;
        mappingStatus = 'error';
      }
    } catch (e) {
      uploadError   = `Upload failed: ${e.message}`;
      isAnalyzing   = false;
      mappingStatus = 'error';
    } finally {
      isUploading = false;
    }
  }
</script>

<div class="anw-root">

  <!-- ── View-masking strip (super admin only) ───────────────────────────────── -->
  {#if auth.user?.role === 'super_admin'}
    <div class="vms-strip">
      <div class="vms-label">
        <span>👁️</span>
        <span>Super Admin Mode: Simulating Interface As</span>
      </div>
      <select class="vms-select" bind:value={currentViewMode}>
        <option value="super_admin">Super Admin (Absolute Controls)</option>
        <option value="admin">Admin User (Staff Upload Template)</option>
        <option value="user">Regular User (Student Study Interface)</option>
      </select>
    </div>
  {/if}

  <!-- Phase rail -->
  <nav class="anw-rail" aria-label="Workflow phases">
    {#each PHASES as p, i}
      <button
        class="rs {phase === p.num ? 'rs--active' : ''} {phase > p.num ? 'rs--done' : ''}"
        onclick={() => (phase = p.num)}
        title="Go to phase {p.num}"
      >
        <span class="rs-num">{phase > p.num ? '✓' : p.num}</span>
        <span class="rs-label">{p.label}</span>
      </button>
      {#if i < PHASES.length - 1}
        <span class="rail-connector {phase > p.num ? 'rc--done' : ''}"></span>
      {/if}
    {/each}
  </nav>

  <!-- Phase body -->
  <div class="anw-body">

    <!-- ── Phase 1: Select Chapter Module ──────────────────────────────────── -->
    {#if phase === 1}
      <div class="phase-panel">
        <div class="phase-hd">
          <h2 class="phase-title">Select Chapter Module</h2>
          <p class="phase-desc">Choose the AFM chapter you are ingesting material for. All uploaded content in Phase 2 will be targeted to this chapter.</p>
        </div>

        <div class="chapter-grid">
          {#each CHAPTERS as ch}
            <button
              class="ch-card {selectedChapter?.id === ch.id ? 'ch-card--selected' : ''}"
              onclick={() => { selectedChapter = ch; phase = 3; }}
            >
              <span class="ch-code">{ch.chapter_code}</span>
              <span class="ch-name">{ch.chapter_name}</span>
              {#if selectedChapter?.id === ch.id}
                <span class="ch-tick">✓</span>
              {/if}
            </button>
          {/each}
        </div>

        <div class="phase-ft">
          <button
            class="btn-primary"
            disabled={!selectedChapter}
            onclick={() => (phase = 2)}
          >
            Proceed to File Ingestion →
          </button>
        </div>
      </div>

    <!-- ── Phase 2: Staged File Ingestion ──────────────────────────────────── -->
    {:else if phase === 2}
      <div class="phase-panel">
        <div class="phase-hd">
          <h2 class="phase-title">Staged File Ingestion</h2>
          <p class="phase-desc">
            Uploading for: <strong class="target-ch">{selectedChapter?.chapter_code} — {selectedChapter?.chapter_name}</strong>
          </p>
        </div>

        {#if currentViewMode !== 'user'}
        {#key dropzoneKey}
          {#if uploadSuccess}
            <div class="success-box">
              <span class="sb-icon">✓</span>
              <div class="sb-body">
                <p class="sb-title">Document ingested successfully</p>
                <p class="sb-meta">
                  {uploadSuccess.total_chunks ?? '—'} chunks mapped ·
                  {uploadSuccess.total_words?.toLocaleString() ?? '—'} words extracted
                </p>
              </div>
              <button
                class="btn-ghost"
                onclick={clearFile}
              >Upload Another</button>
            </div>

            {#if mappingStatus === 'success'}
              <div class="mapping-status mapping-status--success">
                <p class="ms-text">✅ Chapter mapped successfully into CA Final AFM Curriculum!</p>
                <button class="btn-primary ms-cta" onclick={() => (phase = 3)}>
                  ✍️ Proceed to Exam Question Generator
                </button>
              </div>
            {/if}
          {:else}
            <div
              class="dropzone {dragOver ? 'dz--over' : ''} {selectedFile ? 'dz--has-file' : ''}"
              ondragover={(e) => { e.preventDefault(); dragOver = true; }}
              ondragleave={() => (dragOver = false)}
              ondrop={(e) => { e.preventDefault(); dragOver = false; handleFileSelect(e); }}
              role="region"
              aria-label="File upload dropzone"
            >
              <input
                type="file"
                id="anw-file"
                accept=".pdf,.txt"
                class="anw-hidden-input"
                onchange={handleFileSelect}
              />
              {#if !selectedFile}
                <label for="anw-file" class="dz-label">
                  <span class="dz-icon">📥</span>
                  <span class="dz-text">Drag and drop or click to browse</span>
                  <span class="dz-hint">Supports .pdf · .txt</span>
                  <span class="dz-btn">Browse Files</span>
                </label>
              {:else}
                <div class="file-preview w-full">
                  <span class="fp-icon">📄</span>
                  <div class="fp-meta">
                    <div style="display: grid; grid-template-columns: minmax(0, 1fr); width: 100%; max-width: 240px;">
                      <span style="overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: #ffffff; font-weight: 500; font-size: 0.875rem;" title={selectedFile.name}>{selectedFile.name}</span>
                    </div>
                    <span class="fp-size">{(selectedFile.size / 1024).toFixed(1)} KB</span>
                  </div>
                  <button
                    class="fp-remove"
                    onclick={handleRemoveFile}
                    title="Remove"
                  >✕</button>
                </div>
              {/if}
            </div>

            {#if uploadError}
              <p class="upload-error">⚠ {uploadError}</p>
            {/if}

            {#if selectedFile}
              <div class="upload-settings">
                <div class="us-row">
                  <span class="us-label">Target</span>
                  <span class="us-value">{selectedChapter?.chapter_code} · Targeted Mapping</span>
                </div>
                <button
                  class="btn-primary"
                  disabled={isAnalyzing}
                  onclick={triggerUpload}
                >
                  {#if isAnalyzing}
                    <span class="spin"></span> Mapping Document Blocks…
                  {:else}
                    🚀 Analyse & Ingest Document
                  {/if}
                </button>
              </div>
            {/if}

            {#if mappingStatus === 'error'}
              <div class="mapping-status mapping-status--error">
                ❌ Analysis block processing failed. Check network telemetry logs or verify backend container health.
              </div>
            {/if}
          {/if}
        {/key}
        {:else}
          <div class="vms-user-stub">
            <span class="vms-stub-icon">🎓</span>
            <p class="vms-stub-text">Student Study Interface — document upload controls are not visible in this view.</p>
          </div>
        {/if}

        <div class="phase-ft">
          <button class="btn-ghost" onclick={() => (phase = 1)}>← Back</button>
          <button
            class="btn-primary"
            disabled={!canLeavePhase2}
            onclick={() => (phase = 3)}
          >
            Proceed to Quiz Proofing →
          </button>
        </div>
      </div>

    <!-- ── Phase 3: Exam Question Generator & Quiz Proofing ───────────────── -->
    {:else if phase === 3}
      <div class="phase-panel phase-panel--wide">
        <div class="phase-hd">
          <h2 class="phase-title">Exam Question Generator & Quiz Proofing</h2>
          <p class="phase-desc">Preview AI-generated questions for the ingested chapter, then approve or refine from the staging queue below.</p>
        </div>

        <!-- Question browser -->
        <div class="p3-grid">
          <div class="p3-sidebar">
            <h3 class="p3-sidebar-hd">Active Chapter</h3>
            <div class="p3-ch-card">
              📚 {selectedChapter?.chapter_code}<br />
              <span style="font-size:0.78rem; color: rgba(147,197,253,0.7);">{selectedChapter?.chapter_name}</span>
            </div>
            {#if generatedQuestions.length > 0}
              <p class="p3-qcount">{generatedQuestions.length} question{generatedQuestions.length !== 1 ? 's' : ''} loaded</p>
            {/if}
          </div>

          <div class="p3-canvas">
            {#if isFetchingQuestions}
              <div class="p3-loading">
                <span class="spin"></span>
                Querying Turso for generated question sequence…
              </div>
            {:else if generatedQuestions.length === 0}
              <div class="p3-empty">
                📭 No questions found for this chapter yet.<br />
                <span>Upload a document in Phase 2 to generate questions.</span>
              </div>
            {:else}
              {@const q = generatedQuestions[currentQuestionIndex]}
              <div class="mcq-container">
                <div class="mcq-header">
                  <span class="mcq-badge">Question {currentQuestionIndex + 1} of {generatedQuestions.length}</span>
                  <span class="mcq-difficulty">Difficulty: {q.difficulty}</span>
                </div>

                <p class="mcq-text">{q.scenario}</p>

                <div class="options-grid">
                  {#each q.options as option, idx}
                    <button class="option-btn {q.correct_option === option ? (revealExplanation ? 'option-btn--correct' : '') : ''}">
                      <span class="option-prefix">{String.fromCharCode(65 + idx)}.</span>
                      {option.replace(/^Option [A-D]:\s*/i, '')}
                    </button>
                  {/each}
                </div>

                <button class="explanation-toggle" onclick={() => (revealExplanation = !revealExplanation)}>
                  {revealExplanation ? '🔼 Hide Explanation' : '🔽 Reveal Gemini Explanatory Rationale'}
                </button>

                {#if revealExplanation}
                  <div class="explanation-panel">
                    <strong>Strategic Context:</strong><br />
                    {q.explanation}
                  </div>
                {/if}

                <div class="nav-actions">
                  <button
                    class="nav-btn"
                    disabled={currentQuestionIndex === 0}
                    onclick={() => { currentQuestionIndex--; revealExplanation = false; }}
                  >← Previous</button>
                  <span class="nav-progress">{currentQuestionIndex + 1} / {generatedQuestions.length}</span>
                  <button
                    class="nav-btn"
                    disabled={currentQuestionIndex >= generatedQuestions.length - 1}
                    onclick={() => { currentQuestionIndex++; revealExplanation = false; }}
                  >Next →</button>
                </div>
              </div>
            {/if}
          </div>
        </div>

        <!-- Staging queue management -->
        <div class="p3-staging-section">
          <h3 class="p3-staging-hd">Staging Queue — Approve & Refine</h3>
          <AdminReviewPanel {baseApiUrl} />
        </div>

        <div class="phase-ft">
          <button class="btn-ghost" onclick={() => (phase = 2)}>← Back</button>
          <button class="btn-primary" onclick={() => (phase = 4)}>
            Proceed to Hackathon →
          </button>
        </div>
      </div>

    <!-- ── Phase 4: Schedule Live Hackathon Event ───────────────────────────── -->
    {:else if phase === 4}
      <div class="phase-panel">
        <div class="phase-hd">
          <h2 class="phase-title">Schedule Live Hackathon Event</h2>
          <p class="phase-desc">Configure and publish a timed quiz tournament for enrolled students. Students are notified 30 minutes before start.</p>
        </div>

        <div class="form-grid">
          <div class="form-field">
            <label class="form-label" for="hack-title">Event Title</label>
            <input
              id="hack-title"
              class="form-input"
              type="text"
              placeholder="e.g. AFM-CH09 Derivatives Sprint"
              bind:value={hackTitle}
            />
          </div>
          <div class="form-field">
            <label class="form-label" for="hack-date">Scheduled Date & Time</label>
            <input
              id="hack-date"
              class="form-input"
              type="datetime-local"
              bind:value={hackDate}
            />
          </div>
          <div class="form-field">
            <label class="form-label" for="hack-dur">Duration (minutes)</label>
            <input
              id="hack-dur"
              class="form-input"
              type="number"
              min="15"
              max="180"
              step="15"
              bind:value={hackDuration}
            />
          </div>
        </div>

        {#if hackSaved}
          <div class="info-banner">✓ Hackathon event saved. Students will be notified before start time.</div>
        {/if}

        <div class="phase-ft">
          <button class="btn-ghost" onclick={() => (phase = 3)}>← Back</button>
          <button
            class="btn-secondary"
            disabled={!hackTitle || !hackDate}
            onclick={() => (hackSaved = true)}
          >
            Save Event
          </button>
          <button class="btn-primary" onclick={() => (phase = 5)}>
            Proceed to Archive →
          </button>
        </div>
      </div>

    <!-- ── Phase 5: Archive Completed Tournaments ───────────────────────────── -->
    {:else if phase === 5}
      <div class="phase-panel">
        <div class="phase-hd">
          <h2 class="phase-title">Archive Completed Tournaments</h2>
          <p class="phase-desc">Review and archive past hackathon sessions. Archived tournaments are retained for analytics but removed from the active event feed.</p>
        </div>

        <div class="archive-empty">
          <span class="ae-icon">🗂️</span>
          <p class="ae-title">No completed tournaments yet</p>
          <p class="ae-sub">Completed hackathon events will appear here for archiving after their scheduled end time has passed.</p>
        </div>

        <div class="phase-ft">
          <button class="btn-ghost" onclick={() => (phase = 4)}>← Back</button>
        </div>
      </div>
    {/if}

  </div>
</div>

<style>
  /* ── Root ─────────────────────────────────────────────────────────────────── */
  .anw-root {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  /* ── Phase rail ───────────────────────────────────────────────────────────── */
  .anw-rail {
    display: flex;
    align-items: center;
    padding: 20px 24px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.07);
    background: rgba(255, 255, 255, 0.018);
    border-radius: 14px 14px 0 0;
    border: 1px solid rgba(255, 255, 255, 0.07);
    overflow-x: auto;
    gap: 0;
  }

  .rs {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
    background: none;
    border: none;
    cursor: pointer;
    padding: 6px 10px;
    border-radius: 8px;
    transition: background 0.15s;
    flex-shrink: 0;
  }
  .rs:hover { background: rgba(255, 255, 255, 0.04); }

  .rs-num {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.75rem;
    font-weight: 700;
    background: rgba(255, 255, 255, 0.06);
    border: 1.5px solid rgba(255, 255, 255, 0.12);
    color: rgba(255, 255, 255, 0.4);
    transition: all 0.2s;
  }
  .rs-label {
    font-size: 0.65rem;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.3);
    white-space: nowrap;
    transition: color 0.2s;
  }

  .rs--active .rs-num {
    background: rgba(59, 130, 246, 0.2);
    border-color: #3b82f6;
    color: #60a5fa;
  }
  .rs--active .rs-label { color: #60a5fa; }

  .rs--done .rs-num {
    background: rgba(52, 211, 153, 0.15);
    border-color: #34d399;
    color: #34d399;
  }
  .rs--done .rs-label { color: #34d399; }

  .rail-connector {
    flex: 1;
    min-width: 20px;
    height: 1.5px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 1px;
    transition: background 0.2s;
  }
  .rc--done { background: rgba(52, 211, 153, 0.4); }

  /* ── Body ─────────────────────────────────────────────────────────────────── */
  .anw-body {
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-top: none;
    border-radius: 0 0 14px 14px;
    background: rgba(255, 255, 255, 0.012);
    min-height: 400px;
  }

  /* ── Phase panel ──────────────────────────────────────────────────────────── */
  .phase-panel {
    padding: 28px 28px 24px;
    display: flex;
    flex-direction: column;
    gap: 22px;
  }
  .phase-panel--wide { padding-top: 22px; }

  .phase-hd { display: flex; flex-direction: column; gap: 6px; }
  .phase-title {
    font-size: 1.05rem;
    font-weight: 700;
    color: #f0f0ff;
    margin: 0;
  }
  .phase-desc {
    font-size: 0.82rem;
    color: rgba(255, 255, 255, 0.42);
    margin: 0;
    line-height: 1.55;
  }
  .target-ch { color: #60a5fa; font-weight: 600; }

  .phase-ft {
    display: flex;
    align-items: center;
    gap: 12px;
    padding-top: 4px;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    flex-wrap: wrap;
  }

  /* ── Chapter grid ─────────────────────────────────────────────────────────── */
  .chapter-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 10px;
  }

  .ch-card {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 12px 14px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 10px;
    cursor: pointer;
    text-align: left;
    transition: background 0.12s, border-color 0.12s;
  }
  .ch-card:hover {
    background: rgba(255, 255, 255, 0.055);
    border-color: rgba(255, 255, 255, 0.13);
  }
  .ch-card--selected {
    background: rgba(59, 130, 246, 0.1);
    border-color: rgba(59, 130, 246, 0.4);
  }
  .ch-code {
    font-size: 0.67rem;
    font-weight: 700;
    font-family: monospace;
    letter-spacing: 0.05em;
    color: rgba(255, 255, 255, 0.35);
  }
  .ch-card--selected .ch-code { color: #60a5fa; }
  .ch-name {
    font-size: 0.78rem;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.7);
    line-height: 1.4;
  }
  .ch-tick {
    position: absolute;
    top: 8px;
    right: 10px;
    font-size: 0.7rem;
    color: #60a5fa;
    font-weight: 700;
  }

  /* ── Dropzone ─────────────────────────────────────────────────────────────── */
  .dropzone {
    border: 2px dashed rgba(255, 255, 255, 0.12);
    border-radius: 12px;
    padding: 32px 24px;
    transition: border-color 0.15s, background 0.15s;
    cursor: default;
  }
  .dz--over {
    border-color: #3b82f6;
    background: rgba(59, 130, 246, 0.05);
  }
  .dz--has-file {
    border-color: rgba(52, 211, 153, 0.35);
    background: rgba(52, 211, 153, 0.04);
  }

  .anw-hidden-input {
    position: absolute;
    width: 0;
    height: 0;
    opacity: 0;
  }

  .dz-label {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    cursor: pointer;
  }
  .dz-icon   { font-size: 2rem; }
  .dz-text   { font-size: 0.9rem; font-weight: 600; color: rgba(255,255,255,0.7); }
  .dz-hint   { font-size: 0.75rem; color: rgba(255,255,255,0.3); }
  .dz-btn {
    margin-top: 4px;
    padding: 6px 18px;
    border: 1px solid rgba(255,255,255,0.14);
    border-radius: 6px;
    font-size: 0.8rem;
    font-weight: 600;
    color: rgba(255,255,255,0.6);
    background: rgba(255,255,255,0.04);
    transition: background 0.13s, color 0.13s;
  }
  .dz-label:hover .dz-btn {
    background: rgba(255,255,255,0.09);
    color: #fff;
  }

  .file-preview {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .w-full {
    width: 100%;
  }
  .fp-icon { font-size: 1.5rem; }
  .fp-meta {
    flex: 1;
    min-width: 0; /* allows the flex child to shrink below its content width so truncation works */
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow: hidden;
  }
  .fp-size { font-size: 0.72rem; color: rgba(255,255,255,0.35); }
  .fp-remove {
    background: rgba(251,113,133,0.08);
    border: 1px solid rgba(251,113,133,0.2);
    color: #fb7185;
    border-radius: 6px;
    padding: 4px 10px;
    font-size: 0.75rem;
    cursor: pointer;
    transition: background 0.12s;
  }
  .fp-remove:hover { background: rgba(251,113,133,0.18); }

  .upload-error {
    font-size: 0.8rem;
    color: #fb7185;
    margin: 0;
    padding: 8px 12px;
    background: rgba(251,113,133,0.07);
    border-radius: 8px;
    border: 1px solid rgba(251,113,133,0.18);
  }

  .upload-settings {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 14px 16px;
    background: rgba(255,255,255,0.03);
    border: 1px solid rgba(255,255,255,0.07);
    border-radius: 10px;
    flex-wrap: wrap;
  }
  .us-row { display: flex; align-items: center; gap: 8px; }
  .us-label { font-size: 0.7rem; font-weight: 600; letter-spacing: 0.05em; text-transform: uppercase; color: rgba(255,255,255,0.3); }
  .us-value { font-size: 0.82rem; font-weight: 600; color: rgba(255,255,255,0.75); }

  /* ── Success box ──────────────────────────────────────────────────────────── */
  .success-box {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 16px 18px;
    background: rgba(52,211,153,0.08);
    border: 1px solid rgba(52,211,153,0.22);
    border-radius: 10px;
    flex-wrap: wrap;
  }
  .sb-icon { font-size: 1.3rem; color: #34d399; flex-shrink: 0; }
  .sb-body { flex: 1; display: flex; flex-direction: column; gap: 3px; }
  .sb-title { font-size: 0.88rem; font-weight: 600; color: #34d399; margin: 0; }
  .sb-meta  { font-size: 0.75rem; color: rgba(255,255,255,0.45); margin: 0; }

  /* ── Hackathon form ───────────────────────────────────────────────────────── */
  .form-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
  }
  .form-field { display: flex; flex-direction: column; gap: 6px; }
  .form-label {
    font-size: 0.7rem;
    font-weight: 600;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: rgba(255,255,255,0.35);
  }
  .form-input {
    background: rgba(255,255,255,0.04);
    border: 1px solid rgba(255,255,255,0.1);
    border-radius: 8px;
    color: rgba(255,255,255,0.85);
    font-size: 0.88rem;
    padding: 9px 12px;
    font-family: inherit;
    transition: border-color 0.15s;
    width: 100%;
    box-sizing: border-box;
  }
  .form-input:focus {
    outline: none;
    border-color: #3b82f6;
    box-shadow: 0 0 0 3px rgba(59,130,246,0.12);
  }
  .form-input::placeholder { color: rgba(255,255,255,0.2); }

  .info-banner {
    padding: 11px 14px;
    background: rgba(52,211,153,0.08);
    border: 1px solid rgba(52,211,153,0.2);
    border-radius: 8px;
    font-size: 0.82rem;
    color: #34d399;
    font-weight: 500;
  }

  /* ── Archive empty state ──────────────────────────────────────────────────── */
  .archive-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 56px 24px;
    text-align: center;
  }
  .ae-icon  { font-size: 2.8rem; opacity: 0.35; }
  .ae-title { font-size: 0.95rem; font-weight: 600; color: rgba(255,255,255,0.45); margin: 0; }
  .ae-sub   { font-size: 0.78rem; color: rgba(255,255,255,0.25); margin: 0; max-width: 380px; line-height: 1.55; }

  /* ── Buttons ──────────────────────────────────────────────────────────────── */
  .btn-primary {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 9px 20px;
    background: linear-gradient(135deg, #3b82f6, #2563eb);
    color: #fff;
    font-size: 0.85rem;
    font-weight: 600;
    border: none;
    border-radius: 8px;
    cursor: pointer;
    transition: opacity 0.15s, transform 0.1s;
    white-space: nowrap;
  }
  .btn-primary:disabled { opacity: 0.38; cursor: not-allowed; transform: none; }
  .btn-primary:not(:disabled):hover { opacity: 0.88; transform: translateY(-1px); }

  .btn-secondary {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 9px 18px;
    background: rgba(167,139,250,0.12);
    color: #a78bfa;
    font-size: 0.85rem;
    font-weight: 600;
    border: 1px solid rgba(167,139,250,0.3);
    border-radius: 8px;
    cursor: pointer;
    transition: background 0.15s;
    white-space: nowrap;
  }
  .btn-secondary:disabled { opacity: 0.38; cursor: not-allowed; }
  .btn-secondary:not(:disabled):hover { background: rgba(167,139,250,0.22); }

  .btn-ghost {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 9px 16px;
    background: transparent;
    color: rgba(255,255,255,0.45);
    font-size: 0.82rem;
    font-weight: 500;
    border: 1px solid rgba(255,255,255,0.1);
    border-radius: 8px;
    cursor: pointer;
    transition: color 0.15s, border-color 0.15s;
    white-space: nowrap;
  }
  .btn-ghost:hover { color: rgba(255,255,255,0.8); border-color: rgba(255,255,255,0.22); }

  /* ── Spinner ──────────────────────────────────────────────────────────────── */
  .spin {
    display: inline-block;
    width: 12px;
    height: 12px;
    border: 2px solid rgba(255,255,255,0.25);
    border-top-color: #fff;
    border-radius: 50%;
    animation: anw-spin 0.7s linear infinite;
  }
  @keyframes anw-spin { to { transform: rotate(360deg); } }
  @keyframes anw-fade-in { from { opacity: 0; transform: translateY(4px); } to { opacity: 1; transform: none; } }

  /* ── Mapping status panels ────────────────────────────────────────────────── */
  .mapping-status {
    margin-top: 16px;
    padding: 14px 16px;
    border-radius: 10px;
    text-align: center;
    animation: anw-fade-in 0.25s ease;
  }
  .mapping-status--success {
    background: rgba(16,185,129,0.08);
    border: 1px solid rgba(16,185,129,0.25);
  }
  .ms-text {
    color: #34d399;
    font-size: 0.875rem;
    font-weight: 500;
    margin: 0 0 12px;
  }
  .ms-cta { width: 100%; }
  .mapping-status--error {
    background: rgba(239,68,68,0.08);
    border: 1px solid rgba(239,68,68,0.25);
    color: #f87171;
    font-size: 0.875rem;
  }

  /* ── Phase 3: Question browser ───────────────────────────────────────────── */
  .p3-grid { display: grid; grid-template-columns: 260px 1fr; gap: 1.5rem; margin-top: 1rem; }
  .p3-sidebar {
    background: #0b0f19;
    border: 1px solid #1e293b;
    border-radius: 8px;
    padding: 1rem;
    height: fit-content;
  }
  .p3-sidebar-hd {
    color: #64748b;
    font-size: 0.72rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    margin: 0 0 10px;
  }
  .p3-ch-card {
    background: #111026;
    border: 1px solid #3b82f6;
    border-radius: 6px;
    padding: 10px 12px;
    color: #93c5fd;
    font-size: 0.85rem;
    font-weight: 500;
    line-height: 1.5;
  }
  .p3-qcount { font-size: 0.75rem; color: #475569; margin: 10px 0 0; }
  .p3-canvas { display: flex; flex-direction: column; gap: 1rem; }
  .p3-loading {
    display: flex;
    align-items: center;
    gap: 10px;
    color: #94a3b8;
    font-size: 0.9rem;
    padding: 40px 24px;
    justify-content: center;
  }
  .p3-empty {
    text-align: center;
    color: #475569;
    font-size: 0.9rem;
    padding: 48px 24px;
    border: 1px dashed rgba(255,255,255,0.07);
    border-radius: 12px;
    line-height: 2;
  }
  .p3-empty span { font-size: 0.82rem; }
  .mcq-container {
    background: #131b2e;
    border: 1px solid #1e293b;
    border-radius: 12px;
    padding: 28px;
    box-shadow: 0 10px 15px -3px rgba(0,0,0,0.3);
  }
  .mcq-header { display: flex; justify-content: space-between; align-items: center; }
  .mcq-badge {
    display: inline-block;
    font-size: 0.65rem;
    background: #2563eb;
    color: #fff;
    padding: 3px 10px;
    border-radius: 20px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .mcq-difficulty { color: #64748b; font-size: 0.75rem; font-weight: 500; }
  .mcq-text { color: #f8fafc; font-size: 1rem; font-weight: 500; margin: 20px 0 0; line-height: 1.6; }
  .options-grid { display: grid; gap: 10px; margin-top: 16px; }
  .option-btn {
    background: #0b0f19;
    border: 1px solid #1e293b;
    color: #cbd5e1;
    text-align: left;
    padding: 12px 16px;
    border-radius: 8px;
    font-size: 0.88rem;
    cursor: pointer;
    transition: background 0.15s, border-color 0.15s, color 0.15s;
    width: 100%;
  }
  .option-btn:hover { background: #1e293b; border-color: #475569; color: #f8fafc; }
  .option-btn--correct { border-color: #10b981; background: rgba(16,185,129,0.08); color: #a7f3d0; }
  .option-prefix { font-weight: 700; color: #3b82f6; margin-right: 6px; }
  .explanation-toggle {
    background: none;
    border: none;
    color: #3b82f6;
    cursor: pointer;
    font-size: 0.82rem;
    font-weight: 600;
    padding: 0;
    margin-top: 20px;
    display: block;
  }
  .explanation-toggle:hover { color: #60a5fa; }
  .explanation-panel {
    margin-top: 12px;
    background: #060b13;
    border-left: 4px solid #10b981;
    padding: 14px 16px;
    border-radius: 4px;
    color: #a7f3d0;
    font-size: 0.85rem;
    line-height: 1.65;
    animation: anw-fade-in 0.2s ease;
  }
  .nav-actions {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-top: 24px;
    border-top: 1px solid #1e293b;
    padding-top: 20px;
  }
  .nav-btn {
    background: #1e293b;
    border: 1px solid #334155;
    color: #f8fafc;
    padding: 6px 16px;
    border-radius: 6px;
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s;
  }
  .nav-btn:hover:not(:disabled) { background: #334155; }
  .nav-btn:disabled { opacity: 0.3; cursor: not-allowed; }
  .nav-progress { font-size: 0.8rem; color: #64748b; }
  .p3-staging-section { margin-top: 2.5rem; border-top: 1px solid #1e293b; padding-top: 1.5rem; }
  .p3-staging-hd {
    font-size: 0.75rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: #64748b;
    margin: 0 0 1rem;
  }

  /* ── View-masking strip ──────────────────────────────────────────────────── */
  .vms-strip {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    background: #121026;
    border: 1px solid #3b82f6;
    border-radius: 6px;
    padding: 8px 14px;
    margin-bottom: 20px;
    font-size: 0.75rem;
    color: #93c5fd;
    box-shadow: 0 4px 6px -1px rgba(0,0,0,0.3);
  }
  .vms-label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: 500;
  }
  .vms-select {
    background: #0f172a;
    border: 1px solid #1e3a8a;
    color: #f8fafc;
    padding: 4px 8px;
    border-radius: 4px;
    font-size: 0.75rem;
    font-weight: 600;
    cursor: pointer;
    outline: none;
  }
  .vms-select:focus { border-color: #3b82f6; }
  .vms-user-stub {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 36px 24px;
    border: 1px dashed rgba(255,255,255,0.1);
    border-radius: 12px;
    text-align: center;
    animation: anw-fade-in 0.2s ease;
  }
  .vms-stub-icon { font-size: 2rem; }
  .vms-stub-text { color: rgba(255,255,255,0.35); font-size: 0.85rem; margin: 0; }

  /* ── Responsive ───────────────────────────────────────────────────────────── */
  @media (max-width: 700px) {
    .chapter-grid     { grid-template-columns: 1fr; }
    .form-grid        { grid-template-columns: 1fr; }
    .phase-panel      { padding: 18px 16px 16px; }
    .upload-settings  { flex-direction: column; align-items: flex-start; }
  }
</style>
