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

  function clearFile() {
    selectedFile  = null;
    uploadError   = '';
    uploadSuccess = null;
    dropzoneKey++;
  }

  function handleRemoveFile() {
    selectedFile = null;
    uploadError  = '';
    dropzoneKey++;
  }

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
    isUploading = true;
    uploadError = '';

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
      } else {
        uploadError = (await res.text()) || `Upload failed (${res.status})`;
      }
    } catch (e) {
      uploadError = `Upload failed: ${e.message}`;
    } finally {
      isUploading = false;
    }
  }
</script>

<div class="anw-root">

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
              onclick={() => (selectedChapter = ch)}
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
                <div class="file-preview">
                  <span class="fp-icon">📄</span>
                  <div class="fp-meta">
                    <span class="fp-name truncate max-w-[280px] sm:max-w-[360px] block text-white font-medium">{selectedFile.name}</span>
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
                  disabled={isUploading}
                  onclick={triggerUpload}
                >
                  {#if isUploading}
                    <span class="spin"></span> Mapping & Generating MCQs…
                  {:else}
                    🚀 Analyse & Ingest Document
                  {/if}
                </button>
              </div>
            {/if}
          {/if}
        {/key}

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

    <!-- ── Phase 3: Inline AI Quiz Proofing ────────────────────────────────── -->
    {:else if phase === 3}
      <div class="phase-panel phase-panel--wide">
        <div class="phase-hd">
          <h2 class="phase-title">Inline AI Quiz Proofing & Edits</h2>
          <p class="phase-desc">Review generated questions from the staging queue. Improvise, refine, approve or reject each entry before promotion to the live databank.</p>
        </div>

        <AdminReviewPanel {baseApiUrl} />

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
  .fp-icon { font-size: 1.5rem; }
  .fp-meta { flex: 1; display: flex; flex-direction: column; gap: 2px; }
  .fp-name { font-size: 0.85rem; font-weight: 600; color: rgba(255,255,255,0.8); }
  .truncate {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .max-w-\[280px\] {
    max-width: 280px;
  }
  .block {
    display: block;
  }
  .text-white {
    color: #fff !important;
  }
  .font-medium {
    font-weight: 500 !important;
  }
  @media (min-width: 640px) {
    .sm\:max-w-\[360px\] {
      max-width: 360px;
    }
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

  /* ── Responsive ───────────────────────────────────────────────────────────── */
  @media (max-width: 700px) {
    .chapter-grid     { grid-template-columns: 1fr; }
    .form-grid        { grid-template-columns: 1fr; }
    .phase-panel      { padding: 18px 16px 16px; }
    .upload-settings  { flex-direction: column; align-items: flex-start; }
  }
</style>
