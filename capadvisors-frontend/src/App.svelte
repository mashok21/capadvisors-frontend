<script>
  import { onMount } from 'svelte';
  import { auth } from './lib/auth.svelte.js';
  import EditProfile from './lib/components/EditProfile.svelte';
  import Leaderboard from './lib/components/Leaderboard.svelte';

  const baseApiUrl = import.meta.env.VITE_API_URL || 'http://localhost:3000';

  // Svelte 5 Runes state management
  let status = $state("loading"); // "loading" | "connected" | "offline"
  let coverageData = $state([]);
  let isRefreshing = $state(false);
  let connectionError = $state("");

  // Upload state
  let uploadType = $state("BULK");
  let targetedChapterId = $state("");
  let selectedFile = $state(null);
  let isUploading = $state(false);
  let dragOver = $state(false);
  let uploadResult = $state(null);
  let showUploadModal = $state(false);

  // Question Viewer state
  let selectedChapter = $state(null);
  let questions = $state([]);
  let isLoadingQuestions = $state(false);
  let answeredQuestions = $state({}); // map of question_id -> selected_option_text
  let showExplanation = $state({}); // map of question_id -> boolean

  // Chapters list static definitions
  const CHAPTERS = [
    { id: "ch01", chapter_code: "AFM-CH01", chapter_name: "Financial Policy and Corporate Strategy" },
    { id: "ch02", chapter_code: "AFM-CH02", chapter_name: "Risk Management" },
    { id: "ch03", chapter_code: "AFM-CH03", chapter_name: "Advanced Capital Budgeting Decisions" },
    { id: "ch04", chapter_code: "AFM-CH04", chapter_name: "Security Analysis" },
    { id: "ch05", chapter_code: "AFM-CH05", chapter_name: "Security Valuation" },
    { id: "ch06", chapter_code: "AFM-CH06", chapter_name: "Portfolio Management" },
    { id: "ch07", chapter_code: "AFM-CH07", chapter_name: "Securitization" },
    { id: "ch08", chapter_code: "AFM-CH08", chapter_name: "Mutual Funds" },
    { id: "ch09", chapter_code: "AFM-CH09", chapter_name: "Derivatives Analysis and Valuation" },
    { id: "ch10", chapter_code: "AFM-CH10", chapter_name: "Foreign Exchange Exposure and Risk Management" },
    { id: "ch11", chapter_code: "AFM-CH11", chapter_name: "International Financial Management" },
    { id: "ch12", chapter_code: "AFM-CH12", chapter_name: "Interest Rate Risk Management" },
    { id: "ch13", chapter_code: "AFM-CH13", chapter_name: "Business Valuation" },
    { id: "ch14", chapter_code: "AFM-CH14", chapter_name: "Mergers, Acquisitions and Corporate Restructuring" },
    { id: "ch15", chapter_code: "AFM-CH15", chapter_name: "Startup Finance" },
  ];

  // Helper mapping from code to name
  const getChapterNameByCode = (code) => {
    const ch = CHAPTERS.find(c => c.chapter_code === code);
    return ch ? ch.chapter_name : "General Finance Section";
  };

  // Local Mock Database seeds
  const initialMockCoverage = CHAPTERS.map(ch => {
    let mapped_chunks_count = 0;
    let total_word_count = 0;
    let total_questions = 0;

    if (ch.chapter_code === "AFM-CH03") {
      mapped_chunks_count = 2;
      total_word_count = 1850;
      total_questions = 2;
    } else if (ch.chapter_code === "AFM-CH06") {
      mapped_chunks_count = 1;
      total_word_count = 920;
      total_questions = 1;
    } else if (ch.chapter_code === "AFM-CH09") {
      mapped_chunks_count = 3;
      total_word_count = 2710;
      total_questions = 1;
    } else if (ch.chapter_code === "AFM-CH15") {
      mapped_chunks_count = 1;
      total_word_count = 780;
      total_questions = 1;
    }

    return {
      chapter_id: ch.id,
      chapter_code: ch.chapter_code,
      chapter_name: ch.chapter_name,
      mapped_chunks_count,
      total_word_count,
      total_questions
    };
  });

  const initialMockQuestions = {
    "ch03": [
      {
        id: "q_ch03_1",
        chapter_id: "ch03",
        difficulty: "Hard",
        scenario: "Alpha Industries Ltd. is deciding whether to replace an existing machinery. The old machine has a book value of INR 200,000 and can be sold now for INR 80,000. The new machine costs INR 800,000, has a 5-year useful life, and is expected to save INR 220,000 annually in operating costs. Tax rate is 30% and depreciation is straight-line. Capital gains/losses are taxed at the same 30% rate. What is the Initial Cash Outflow (Net Investment) at Year 0?",
        options: [
          "Option A: INR 800,000",
          "Option B: INR 684,000",
          "Option C: INR 720,000",
          "Option D: INR 656,000"
        ],
        correct_option: "Option B: INR 684,000",
        explanation: "Initial cost = INR 800,000. Sale of old machine = INR 80,000. Book value of old = INR 200,000, generating a tax loss of INR 120,000 (200,000 - 80,000). Tax shield on loss = 120,000 * 30% = INR 36,000. Net Outflow = 800,000 - 80,000 - 36,000 = INR 684,000."
      },
      {
        id: "q_ch03_2",
        chapter_id: "ch03",
        difficulty: "Medium",
        scenario: "When conducting sensitivity analysis for advanced capital budgeting, which of the following variables is typically considered the most critical if the company operates under highly volatile demand conditions?",
        options: [
          "Option A: Initial Outflow",
          "Option B: Cost of Capital",
          "Option C: Sales Volume",
          "Option D: Salvage Value"
        ],
        correct_option: "Option C: Sales Volume",
        explanation: "Sales volume sensitivity directly scales cash inflows. Under high market volatility, minor percentage drops in Sales Volume often trigger severe shifts in NPV compared to fixed initial costs."
      }
    ],
    "ch06": [
      {
        id: "q_ch06_1",
        chapter_id: "ch06",
        difficulty: "Medium",
        scenario: "An investor holds a portfolio consisting of Stock X (Weight 60%, Beta 1.4) and Stock Y (Weight 40%, Beta 0.8). If the risk-free rate of return is 6% and the expected return on the market portfolio is 12%, calculate the expected return of this portfolio under CAPM.",
        options: [
          "Option A: 12.96%",
          "Option B: 11.60%",
          "Option C: 13.08%",
          "Option D: 10.45%"
        ],
        correct_option: "Option A: 12.96%",
        explanation: "Portfolio Beta = (0.6 * 1.4) + (0.4 * 0.8) = 0.84 + 0.32 = 1.16. Portfolio Expected Return = Rf + Beta * (Rm - Rf) = 6% + 1.16 * (12% - 6%) = 6% + 1.16 * 6% = 6% + 6.96% = 12.96%."
      }
    ],
    "ch09": [
      {
        id: "q_ch09_1",
        chapter_id: "ch09",
        difficulty: "Hard",
        scenario: "A trader sells a European call option with a strike price of INR 500 for a premium of INR 35. At the same time, the trader buys a call option with a strike price of INR 550 for a premium of INR 12 on the same underlying stock. What is the maximum profit and maximum loss of this bull call spread for the options buyer (assuming the reverse positions are bought)?",
        options: [
          "Option A: Max Profit = INR 27, Max Loss = INR 23",
          "Option B: Max Profit = INR 35, Max Loss = INR 15",
          "Option C: Max Profit = INR 38, Max Loss = INR 12",
          "Option D: Max Profit = INR 23, Max Loss = INR 27"
        ],
        correct_option: "Option A: Max Profit = INR 27, Max Loss = INR 23",
        explanation: "Net Premium paid = Call bought (12) - Call sold (0) - wait, for a bull call spread, we buy lower strike (strike 500 premium 35) and sell higher strike (strike 550 premium 12). Net cost = 35 - 12 = INR 23. This is the maximum loss. Maximum spread value = 550 - 500 = INR 50. Maximum profit = Max spread (50) - Net Premium (23) = INR 27."
      }
    ],
    "ch15": [
      {
        id: "q_ch15_1",
        chapter_id: "ch15",
        difficulty: "Medium",
        scenario: "In venture capital financing, which funding round is primarily intended to help a startup scale operations, expand marketing budgets, and begin international expansion after product-market fit has been established?",
        options: [
          "Option A: Seed Round",
          "Option B: Series B Round",
          "Option C: Angel Round",
          "Option D: Bridge Round"
        ],
        correct_option: "Option B: Series B Round",
        explanation: "Series B rounds are focused on scaling the business once product-market fit is proven (which occurs in Seed/Series A). Angel and Seed rounds are for initial development."
      }
    ]
  };

  // Setup client local storage to support offline mock state
  const loadLocalData = () => {
    const savedCoverage = localStorage.getItem("capadvisors_coverage");
    const savedQuestions = localStorage.getItem("capadvisors_questions");

    if (!savedCoverage) {
      localStorage.setItem("capadvisors_coverage", JSON.stringify(initialMockCoverage));
      coverageData = initialMockCoverage;
    } else {
      coverageData = JSON.parse(savedCoverage);
    }

    if (!savedQuestions) {
      localStorage.setItem("capadvisors_questions", JSON.stringify(initialMockQuestions));
    }
  };

  const checkBackendStatus = async () => {
    isRefreshing = true;
    connectionError = "";
    try {
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), 4000);
      const res = await fetch(`${baseApiUrl}/api/hello`, { signal: controller.signal });
      clearTimeout(timeoutId);

      if (res.ok) {
        status = "connected";
        await fetchLiveCoverage();
      } else {
        throw new Error("Offline response");
      }
    } catch (e) {
      console.warn("Backend offline.", e);
      status = "offline";
      connectionError = `Unable to connect to the live backend server at ${baseApiUrl}. Please ensure it is running on port 3000.`;
      
      // Load empty stats
      coverageData = CHAPTERS.map(ch => ({
        chapter_id: ch.id,
        chapter_code: ch.chapter_code,
        chapter_name: ch.chapter_name,
        mapped_chunks_count: 0,
        total_word_count: 0,
        total_questions: 0
      }));
    } finally {
      isRefreshing = false;
    }
  };

  const fetchLiveCoverage = async () => {
    connectionError = "";
    try {
      const res = await fetch(`${baseApiUrl}/api/nexus/coverage`);
      if (res.ok) {
        const data = await res.json();
        // Map backend chapter records to matches
        coverageData = data.map(item => ({
          chapter_id: item.chapter_id,
          chapter_code: item.chapter_code,
          chapter_name: item.chapter_name,
          mapped_chunks_count: item.mapped_chunks_count,
          total_word_count: item.total_word_count,
          total_questions: item.total_questions
        }));
      } else {
        throw new Error(`HTTP error! status: ${res.status}`);
      }
    } catch (e) {
      console.error("Failed to load live coverage:", e);
      connectionError = `Failed to load syllabus coverage statistics: ${e.message}`;
    }
  };

  // Fetch Questions for a specific chapter
  const loadQuestions = async (chapter) => {
    selectedChapter = chapter;
    answeredQuestions = {};
    showExplanation = {};
    isLoadingQuestions = true;
    connectionError = "";

    try {
      const res = await fetch(`${baseApiUrl}/api/nexus/chapters/${chapter.chapter_id}/questions`);
      if (res.ok) {
        questions = await res.json();
      } else {
        throw new Error(`HTTP error! status: ${res.status}`);
      }
    } catch (e) {
      console.error("Failed to fetch questions from API:", e);
      connectionError = `Error fetching questions for ${chapter.chapter_code}: ${e.message}`;
      questions = [];
    } finally {
      isLoadingQuestions = false;
    }
  };

  const handleFileSelect = (e) => {
    const files = e.target.files || e.dataTransfer.files;
    if (files && files.length > 0) {
      selectedFile = files[0];
    }
  };

  const triggerUpload = async () => {
    if (!selectedFile) return;
    isUploading = true;
    connectionError = "";

    // Real API Multipart Upload
    const formData = new FormData();
    formData.append("file", selectedFile);
    formData.append("upload_type", uploadType);
    if (uploadType === "TARGETED") {
      formData.append("chapter_id", targetedChapterId);
    }

    try {
      const res = await fetch(`${baseApiUrl}/api/nexus/upload`, {
        method: "POST",
        body: formData
      });

      if (res.ok) {
        const data = await res.json();
        // Map to results view
        uploadResult = {
          fileName: data.file_name,
          totalChunks: data.total_chunks,
          totalWords: data.total_words,
          mappings: data.mappings.map((m, idx) => ({
            index: idx + 1,
            chunkId: m.chunk_id,
            chapterCode: m.chapter_code,
            confidence: m.confidence,
            questionsCount: m.questions_generated
          }))
        };
        showUploadModal = true;
        selectedFile = null;
        await fetchLiveCoverage();
      } else {
        const errMsg = await res.text();
        throw new Error(errMsg || `Upload failed with status ${res.status}`);
      }
    } catch (e) {
      console.error("API upload failed:", e);
      connectionError = `File upload failed: ${e.message}`;
    } finally {
      isUploading = false;
    }
  };

  // Mock Upload processing logic
  const simulateOfflineUpload = () => {
    const name = selectedFile.name;
    const totalWords = Math.floor(Math.random() * 2000) + 1000;
    const chunkCount = Math.ceil(totalWords / 900);
    
    const mappings = [];
    const localQMap = JSON.parse(localStorage.getItem("capadvisors_questions") || "{}");
    const localCov = JSON.parse(localStorage.getItem("capadvisors_coverage") || "[]");

    // Preselected chapter code if TARGETED
    let preselectedCode = "";
    let preselectedId = "";
    if (uploadType === "TARGETED" && targetedChapterId) {
      const match = CHAPTERS.find(c => c.id === targetedChapterId);
      if (match) {
        preselectedCode = match.chapter_code;
        preselectedId = match.id;
      }
    }

    for (let i = 0; i < chunkCount; i++) {
      let mappedCode = preselectedCode;
      let mappedId = preselectedId;

      if (!mappedCode) {
        // Randomly assign to a chapter representing study material topics
        const randomIndex = Math.floor(Math.random() * CHAPTERS.length);
        mappedCode = CHAPTERS[randomIndex].chapter_code;
        mappedId = CHAPTERS[randomIndex].id;
      }

      const chunkId = "chunk_" + Math.random().toString(36).substring(2, 9);
      const confidence = parseFloat((0.75 + Math.random() * 0.23).toFixed(2));
      const questionsCount = Math.floor(Math.random() * 2) + 1; // 1 to 2 questions

      mappings.push({
        index: i + 1,
        chunkId,
        chapterCode: mappedCode,
        confidence,
        questionsCount
      });

      // Generate mock questions
      const generatedQs = [];
      for (let j = 0; j < questionsCount; j++) {
        const qId = `q_gen_${Math.random().toString(36).substring(2, 9)}`;
        generatedQs.push({
          id: qId,
          chapter_id: mappedId,
          difficulty: Math.random() > 0.5 ? "Hard" : "Medium",
          scenario: `[Generated Scenario for ${mappedCode}] A company is evaluating corporate strategic factors related to ${getChapterNameByCode(mappedCode)}. Calculate standard risk weights, discount returns, or hedging matrices under CA Final AFM syllabus requirements.`,
          options: [
            "Option A: Execute portfolio rebalancing to reduce systemic risk.",
            "Option B: Leverage derivatives hedging matching current cash outflows.",
            "Option C: Structure an SPV transfer for synthetic credit enhancements.",
            "Option D: Shift capital structure towards optimal debt limits."
          ],
          correct_option: "Option B: Leverage derivatives hedging matching current cash outflows.",
          explanation: "Option B represents the standard optimal solution according to ICAI syllabus equations for risk hedging."
        });
      }

      // Add to mock Q database
      if (!localQMap[mappedId]) {
        localQMap[mappedId] = [];
      }
      localQMap[mappedId].push(...generatedQs);

      // Update coverage stats
      const covIndex = localCov.findIndex(c => c.chapter_id === mappedId);
      if (covIndex !== -1) {
        localCov[covIndex].mapped_chunks_count += 1;
        localCov[covIndex].total_word_count += Math.floor(totalWords / chunkCount);
        localCov[covIndex].total_questions += questionsCount;
      }
    }

    // Save states back to local storage
    localStorage.setItem("capadvisors_questions", JSON.stringify(localQMap));
    localStorage.setItem("capadvisors_coverage", JSON.stringify(localCov));

    coverageData = localCov;
    uploadResult = {
      fileName: name,
      totalChunks: chunkCount,
      totalWords,
      mappings
    };

    isUploading = false;
    selectedFile = null;
    showUploadModal = true;
  };

  const resetMockDatabase = () => {
    localStorage.removeItem("capadvisors_coverage");
    localStorage.removeItem("capadvisors_questions");
    loadLocalData();
  };

  // Derive sums
  let mappedChaptersCount = $derived(coverageData.filter(c => c.mapped_chunks_count > 0).length);
  let totalChunksSum = $derived(coverageData.reduce((acc, curr) => acc + curr.mapped_chunks_count, 0));
  let totalWordsSum = $derived(coverageData.reduce((acc, curr) => acc + curr.total_word_count, 0));
  let totalQuestionsSum = $derived(coverageData.reduce((acc, curr) => acc + curr.total_questions, 0));

  onMount(() => {
    checkBackendStatus();
  });

  // Active view
  let activeView = $state('nexus'); // 'nexus' | 'leaderboard'

  // Auth form state
  let authEmail = $state('');
  let authPassword = $state('');
  let authIsRegistering = $state(false);
  let authIsLoading = $state(false);
  let authError = $state('');

  async function handleAuthSubmit(e) {
    e.preventDefault();
    authIsLoading = true;
    authError = '';
    const endpoint = authIsRegistering ? '/api/auth/register' : '/api/auth/login';
    try {
      const res = await fetch(`${baseApiUrl}${endpoint}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email: authEmail, password: authPassword })
      });
      if (res.ok) {
        const data = await res.json();
        auth.login(data.token, { id: data.user_id, role: data.role });
      } else {
        const msg = await res.text();
        authError = msg || 'Authentication failed. Please check your credentials.';
      }
    } catch (_) {
      authError = 'Could not reach the server. Please ensure the backend is running.';
    } finally {
      authIsLoading = false;
    }
  }
</script>

{#if !auth.isAuthenticated}
<main class="auth-container">
  <div class="auth-card">
    <div class="auth-header">
      <div class="auth-logo-mark">CAPADVISORS</div>
      <h2 class="auth-title">{authIsRegistering ? 'Create Account' : 'Welcome Back'}</h2>
      <p class="auth-subtitle">CA Final AFM Nexus Platform</p>
    </div>

    {#if authError}
      <div class="auth-error-banner">{authError}</div>
    {/if}

    <form class="auth-form" onsubmit={handleAuthSubmit}>
      <div class="auth-field">
        <label for="auth-email">Email Address</label>
        <input
          id="auth-email"
          type="email"
          bind:value={authEmail}
          placeholder="your@email.com"
          required
          autocomplete="email"
        />
      </div>
      <div class="auth-field">
        <label for="auth-password">Password</label>
        <input
          id="auth-password"
          type="password"
          bind:value={authPassword}
          placeholder="••••••••"
          required
          autocomplete={authIsRegistering ? 'new-password' : 'current-password'}
        />
      </div>
      <button type="submit" class="auth-submit-btn" disabled={authIsLoading}>
        {#if authIsLoading}
          <span class="spinner"></span>
          {authIsRegistering ? 'Creating account...' : 'Signing in...'}
        {:else}
          {authIsRegistering ? 'Create Account' : 'Sign In'}
        {/if}
      </button>
    </form>

    <p class="auth-toggle-row">
      {authIsRegistering ? 'Already have an account?' : "Don't have an account?"}
      <button
        type="button"
        class="auth-toggle-btn"
        onclick={() => { authIsRegistering = !authIsRegistering; authError = ''; }}
      >
        {authIsRegistering ? 'Sign In' : 'Register'}
      </button>
    </p>
  </div>
</main>
{:else}
<div class="app-container">
  <div class="bg-glow"></div>

  {#if connectionError}
    <div class="connection-error-toast animate-slide" id="connection-error-toast">
      <span class="error-icon">⚠️</span>
      <span class="error-msg">{connectionError}</span>
      <button class="dismiss-btn" onclick={() => connectionError = ""}>✕</button>
    </div>
  {/if}

  <!-- Header -->
  <header class="app-header">
    <div class="logo-wrapper">
      <div class="logo-inner">
        <span class="logo-text">CAPADVISORS</span>
      </div>
    </div>
    <h1 class="app-title" id="main-heading">
      Nexus <span>Mapping Dashboard</span>
    </h1>
    <p class="app-subtitle">
      Parse CA Final study material, classify concepts across the Advanced Financial Management (AFM) syllabus, and generate high-yield mock exam questions using generative intelligence.
    </p>
  </header>

  <!-- Connection / Control Banner -->
  <div class="connection-banner">
    <div class="status-indicator">
      {#if status === "loading"}
        <div class="status-badge loading" id="badge-loading">
          <span class="spinner"></span>
          Scanning system...
        </div>
      {:else if status === "connected"}
        <div class="status-badge connected" id="badge-connected">
          <span class="status-dot"></span>
          Live Railway API
        </div>
      {:else if status === "offline"}
        <div class="status-badge offline" id="badge-offline">
          <span class="status-dot"></span>
          Local Demo Mode
        </div>
      {/if}
    </div>

    <div class="banner-actions">
      <!-- View tabs -->
      <div class="view-tabs">
        <button
          class="view-tab {activeView === 'nexus' ? 'active' : ''}"
          onclick={() => activeView = 'nexus'}
        >📊 Nexus</button>
        <button
          class="view-tab {activeView === 'leaderboard' ? 'active' : ''}"
          onclick={() => activeView = 'leaderboard'}
        >🏆 Rankings</button>
        <button
          class="view-tab {activeView === 'profile' ? 'active' : ''}"
          onclick={() => activeView = 'profile'}
        >👤 Profile</button>
      </div>

      <button
        id="btn-refresh"
        class="action-btn icon-btn"
        onclick={checkBackendStatus}
        disabled={isRefreshing || status === "loading"}
        title="Check Backend Connection"
      >
        {#if isRefreshing}
          <span class="spinner"></span>
        {:else}
          🔄
        {/if}
        Refresh
      </button>

      {#if status === "offline"}
        <button
          class="action-btn danger-btn"
          onclick={resetMockDatabase}
        >
          🧹 Reset Demo DB
        </button>
      {/if}
      <button
        class="action-btn logout-btn"
        onclick={() => auth.logout()}
        title="Sign Out"
      >
        ⏻ Sign Out
      </button>
    </div>
  </div>

  <!-- Global Metrics Summary -->
  <section class="global-stats-row">
    <div class="stat-box">
      <span class="stat-label">Mapped Chapters</span>
      <span class="stat-val">{mappedChaptersCount} / 15</span>
    </div>
    <div class="stat-box">
      <span class="stat-label">Extracted Chunks</span>
      <span class="stat-val">{totalChunksSum}</span>
    </div>
    <div class="stat-box font-gradient">
      <span class="stat-label">Word Count Mapped</span>
      <span class="stat-val">{totalWordsSum.toLocaleString()}</span>
    </div>
    <div class="stat-box">
      <span class="stat-label">Questions Generated</span>
      <span class="stat-val">{totalQuestionsSum}</span>
    </div>
  </section>

  <!-- Main content: swap between Nexus and Leaderboard -->
  {#if activeView === 'leaderboard'}
    <div class="leaderboard-panel">
      <Leaderboard {baseApiUrl} />
    </div>
  {/if}

  {#if activeView === 'profile'}
    <div class="profile-panel">
      <EditProfile {baseApiUrl} />
    </div>
  {/if}

  <!-- Main Grid Layout -->
  <div class="dashboard-grid" class:hidden={activeView !== 'nexus'}>
    <!-- Left Column: Coverage List -->
    <main class="dashboard-section main-coverage" id="status-dashboard">
      <div class="section-header">
        <h2 class="section-title">AFM Chapter Coverage</h2>
        <span class="section-desc">Click a chapter card to browse generated exam questions</span>
      </div>

      <div class="chapters-grid">
        {#each coverageData as chapter}
          <button 
            type="button" 
            class="chapter-card {chapter.mapped_chunks_count > 0 ? 'active' : 'empty'}"
            onclick={() => loadQuestions(chapter)}
            id="card-{chapter.chapter_code}"
          >
            <div class="card-top">
              <span class="chapter-code">{chapter.chapter_code}</span>
              {#if chapter.mapped_chunks_count > 0}
                <span class="card-indicator">Mapped</span>
              {:else}
                <span class="card-indicator outline">Empty</span>
              {/if}
            </div>

            <h3 class="chapter-title">{chapter.chapter_name}</h3>

            <div class="card-metrics">
              <div class="card-metric">
                <span class="metric-lbl">Chunks</span>
                <span class="metric-val">{chapter.mapped_chunks_count}</span>
              </div>
              <div class="card-metric">
                <span class="metric-lbl">Words</span>
                <span class="metric-val">{chapter.total_word_count}</span>
              </div>
              <div class="card-metric highlighted">
                <span class="metric-lbl">MCQs</span>
                <span class="metric-val">{chapter.total_questions}</span>
              </div>
            </div>

            {#if chapter.total_word_count > 0}
              <div class="progress-bar-wrapper">
                <div class="progress-bar-fill" style="width: {Math.min(100, (chapter.total_word_count / 3000) * 100)}%"></div>
              </div>
            {/if}
          </button>
        {/each}
      </div>
    </main>

    <!-- Right Column: Document Upload Controls -->
    <aside class="dashboard-section side-controls">
      <div class="section-header">
        <h2 class="section-title">Upload Study Document</h2>
        <span class="section-desc">Submit a PDF chapter or text chunk to trigger mapping and classification</span>
      </div>

      <!-- Drag & Drop Upload Zone -->
      <div 
        class="upload-dropzone {dragOver ? 'dragover' : ''} {selectedFile ? 'has-file' : ''}"
        ondragover={(e) => { e.preventDefault(); dragOver = true; }}
        ondragleave={() => dragOver = false}
        ondrop={(e) => { e.preventDefault(); dragOver = false; handleFileSelect(e); }}
        role="region"
        aria-label="File Upload Dropzone"
      >
        <input 
          type="file" 
          id="file-input" 
          accept=".pdf,.txt" 
          class="hidden-file-input"
          onchange={handleFileSelect} 
        />
        
        {#if !selectedFile}
          <label for="file-input" class="dropzone-label">
            <span class="upload-icon">📥</span>
            <span class="upload-text-bold">Drag and drop file here</span>
            <span class="upload-text-small">Supports .pdf, .txt documents</span>
            <span class="btn-browse">Browse Files</span>
          </label>
        {:else}
          <div class="file-preview-box">
            <span class="file-icon">📄</span>
            <div class="file-details">
              <span class="file-name">{selectedFile.name}</span>
              <span class="file-size">{(selectedFile.size / 1024).toFixed(1)} KB</span>
            </div>
            <button 
              type="button" 
              class="btn-remove-file" 
              onclick={() => selectedFile = null}
              title="Remove File"
            >
              ✕
            </button>
          </div>
        {/if}
      </div>

      <!-- Settings Form -->
      <div class="upload-settings">
        <div class="form-group">
          <label class="form-label" for="select-type">Mapping Analysis Protocol</label>
          <div class="radio-group">
            <button 
              type="button" 
              class="radio-btn {uploadType === 'BULK' ? 'selected' : ''}" 
              onclick={() => uploadType = "BULK"}
            >
              🔄 Bulk Auto-Classify
            </button>
            <button 
              type="button" 
              class="radio-btn {uploadType === 'TARGETED' ? 'selected' : ''}" 
              onclick={() => uploadType = "TARGETED"}
            >
              🎯 Targeted Chapter
            </button>
          </div>
        </div>

        {#if uploadType === "TARGETED"}
          <div class="form-group animate-slide">
            <label class="form-label" for="select-chapter">Target Chapter Destination</label>
            <select 
              id="select-chapter" 
              class="form-select" 
              bind:value={targetedChapterId}
            >
              <option value="">-- Choose Chapter --</option>
              {#each CHAPTERS as ch}
                <option value={ch.id}>{ch.chapter_code}: {ch.chapter_name}</option>
              {/each}
            </select>
          </div>
        {/if}

        <button 
          id="btn-upload" 
          class="upload-trigger-btn"
          disabled={!selectedFile || isUploading || (uploadType === "TARGETED" && !targetedChapterId)}
          onclick={triggerUpload}
        >
          {#if isUploading}
            <span class="spinner"></span> Mapping & Generating MCQs...
          {:else}
            🚀 Analyze & Map Document
          {/if}
        </button>
      </div>

      <!-- Info Box -->
      <div class="info-note-box">
        <h4>💡 Processing Architecture</h4>
        <p>
          Documents are divided into sequential ~900-word blocks. Each block is sent to the Gemini API to map it onto one of the 15 chapters of the CA Final AFM curriculum and generate 2 to 3 exam-difficulty scenario questions with logical explanations.
        </p>
      </div>
    </aside>
  </div>

  <!-- Modal: Upload Summary Results -->
  {#if showUploadModal && uploadResult}
    <div class="modal-backdrop">
      <div class="modal-card scale-up" id="upload-results-modal">
        <div class="modal-header">
          <h3 class="modal-title">Mapping Summary</h3>
          <button type="button" class="close-btn" onclick={() => showUploadModal = false}>✕</button>
        </div>

        <div class="modal-body">
          <div class="upload-meta-grid">
            <div class="meta-item">
              <span class="meta-lbl">Source Document</span>
              <span class="meta-val">{uploadResult.fileName}</span>
            </div>
            <div class="meta-item">
              <span class="meta-lbl">Words Extracted</span>
              <span class="meta-val">{uploadResult.totalWords.toLocaleString()}</span>
            </div>
            <div class="meta-item">
              <span class="meta-lbl">Blocks Mapped</span>
              <span class="meta-val">{uploadResult.totalChunks}</span>
            </div>
          </div>

          <h4 class="results-table-title">Chunk Mapping Details</h4>
          <div class="results-table-wrapper">
            <table class="results-table">
              <thead>
                <tr>
                  <th>Chunk</th>
                  <th>Mapped Chapter Code</th>
                  <th>Mapped Chapter Title</th>
                  <th>Confidence</th>
                  <th>Questions</th>
                </tr>
              </thead>
              <tbody>
                {#each uploadResult.mappings as mapping}
                  <tr>
                    <td>#{mapping.index}</td>
                    <td><span class="table-badge-code">{mapping.chapterCode}</span></td>
                    <td><span class="table-chapter-title">{getChapterNameByCode(mapping.chapterCode)}</span></td>
                    <td>
                      <div class="confidence-indicator">
                        <span class="confidence-bar" style="width: {mapping.confidence * 100}%"></span>
                        <span class="confidence-text">{(mapping.confidence * 100).toFixed(0)}%</span>
                      </div>
                    </td>
                    <td><span class="table-questions-count">+{mapping.questionsCount} MCQs</span></td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        </div>

        <div class="modal-footer">
          <button type="button" class="action-btn primary-btn" onclick={() => showUploadModal = false}>Close Summary</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Modal/Drawer: Chapter Questions & Quiz -->
  {#if selectedChapter}
    <div class="modal-backdrop">
      <div class="modal-card wide-card slide-up" id="questions-modal">
        <div class="modal-header">
          <div class="modal-title-group">
            <span class="modal-subtitle">{selectedChapter.chapter_code}</span>
            <h3 class="modal-title">{selectedChapter.chapter_name}</h3>
          </div>
          <button type="button" class="close-btn" onclick={() => selectedChapter = null}>✕</button>
        </div>

        <div class="modal-body scrollable">
          {#if isLoadingQuestions}
            <div class="modal-loading-state">
              <span class="spinner large"></span>
              <p>Retrieving high-yield questions from database...</p>
            </div>
          {:else if questions.length === 0}
            <div class="modal-empty-state">
              <span class="empty-icon">📭</span>
              <p class="empty-bold">No questions generated yet</p>
              <p class="empty-sub">Upload relevant study text or chapters in the right panel to extract chunks and auto-generate questions for this chapter.</p>
            </div>
          {:else}
            <div class="quiz-progress-summary">
              Generated Questions count: <strong>{questions.length} Scenario-based MCQs</strong>
            </div>

            <div class="questions-list">
              {#each questions as question, index}
                <div class="question-block">
                  <div class="q-block-header">
                    <span class="q-number">Question {index + 1}</span>
                    <span class="q-difficulty {question.difficulty.toLowerCase()}">{question.difficulty}</span>
                  </div>

                  <p class="q-scenario">{question.scenario}</p>

                  <div class="q-options">
                    {#each question.options as option}
                      <button
                        type="button"
                        class="option-row {answeredQuestions[question.id] === option ? (option === question.correct_option ? 'correct' : 'incorrect') : ''} {answeredQuestions[question.id] && option === question.correct_option ? 'correct-reveal' : ''}"
                        onclick={() => {
                          if (!answeredQuestions[question.id]) {
                            answeredQuestions[question.id] = option;
                          }
                        }}
                        disabled={!!answeredQuestions[question.id]}
                      >
                        <span class="option-check-dot"></span>
                        <span class="option-text">{option}</span>
                      </button>
                    {/each}
                  </div>

                  {#if answeredQuestions[question.id]}
                    <div class="q-feedback animate-fade">
                      {#if answeredQuestions[question.id] === question.correct_option}
                        <div class="feedback-indicator success">
                          ✓ Correct Answer Selected
                        </div>
                      {:else}
                        <div class="feedback-indicator error">
                          ✗ Incorrect Option Selected
                        </div>
                      {/if}

                      <button 
                        type="button" 
                        class="toggle-explanation-btn" 
                        onclick={() => showExplanation[question.id] = !showExplanation[question.id]}
                      >
                        {showExplanation[question.id] ? "Hide Detailed Explanation" : "Reveal Formulas & Detailed Explanation"}
                      </button>

                      {#if showExplanation[question.id]}
                        <div class="explanation-box animate-slide">
                          <h5>Mathematical & Logical Explanation</h5>
                          <p>{question.explanation}</p>
                        </div>
                      {/if}
                    </div>
                  {/if}
                </div>
              {/each}
            </div>
          {/if}
        </div>

        <div class="modal-footer">
          <button type="button" class="action-btn secondary-btn" onclick={() => selectedChapter = null}>Close Viewer</button>
        </div>
      </div>
    </div>
  {/if}

  <footer class="app-footer">
    © 2026 Capadvisors. Crafted for premium asset & learning analytics. Powered by <span class="badge svelte">Svelte 5</span> & <span class="badge rust">Rust Axum</span>.
  </footer>
</div>
{/if}
