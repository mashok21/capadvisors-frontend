<script>
  import { onMount } from 'svelte';
  import { fade } from 'svelte/transition';
  import { auth } from '../auth.svelte.js';

  let { baseApiUrl = '' } = $props();

  let entries    = $state([]);
  let loading    = $state(true);
  let error      = $state('');
  let verifiedOnly = $state(false);

  // Per-row heatmap drawer state
  let expandedRow    = $state(null);        // student_id of the open row
  let activityCache  = $state({});          // { [student_id]: ActivityDay[] }
  let activityLoading = $state({});         // { [student_id]: boolean }
  let hoveredCell    = $state(null);        // { date, count } for tooltip

  const RD_THRESHOLD = 80;
  const MEDALS       = ['🥇', '🥈', '🥉'];
  const GAUGE_R      = 36;
  const GAUGE_C      = 2 * Math.PI * GAUGE_R; // ≈ 226.2

  const TIER = {
    'Strategic Master':    { cls: 'tier-master',   icon: '♛', ring: '#ffc400' },
    'Advanced Analyst':    { cls: 'tier-advanced',  icon: '◆', ring: '#60a5fa' },
    'Senior Candidate':    { cls: 'tier-senior',    icon: '●', ring: '#34d399' },
    'Novice Practitioner': { cls: 'tier-novice',    icon: '○', ring: 'rgba(255,255,255,0.2)' },
  };

  const BADGE_COLOR = {
    'Corp Strategy':  '#e879f9', 'Risk Mgmt':     '#f97316', 'Cap Budget':    '#38bdf8',
    'Sec Analysis':   '#818cf8', 'Valuation':     '#60a5fa', 'Portfolio':     '#34d399',
    'Securitization': '#67e8f9', 'Mutual Funds':  '#a3e635', 'Derivatives':   '#a78bfa',
    'Forex':          '#f0c040', 'Intl Finance':  '#fbbf24', 'Interest Rate': '#c084fc',
    'Biz Valuation':  '#fb923c', 'M&A':           '#fb7185', 'Startup':       '#86efac',
    'General':        'rgba(255,255,255,0.25)',
  };

  // Derived state
  let displayed = $derived(
    verifiedOnly ? entries.filter(e => e.rating_deviation <= RD_THRESHOLD) : entries
  );

  let myEntry = $derived(
    auth.user ? entries.find(e => e.student_id === auth.user.id) ?? null : null
  );

  // ── Helpers ────────────────────────────────────────────────────────────────

  function initials(name) {
    if (!name) return '??';
    return name.trim().split(/\s+/).map(w => w[0] ?? '').join('').toUpperCase().slice(0, 2);
  }

  function rdStatus(rd) {
    if (rd <= 80)  return { label: 'Verified',   cls: 'rd-verified' };
    if (rd <= 150) return { label: 'Active',      cls: 'rd-active' };
    return             { label: 'Provisional', cls: 'rd-provisional' };
  }

  function topPct(entry) {
    const rank = entry.national_rank ?? 1;
    return Math.max(1, Math.ceil((rank / Math.max(entries.length, 1)) * 100));
  }

  function accuracyPct(entry) {
    if (!entry.accuracy_total) return 0;
    return entry.accuracy_correct / entry.accuracy_total;
  }

  // ── Data fetching ──────────────────────────────────────────────────────────

  async function fetchLeaderboard() {
    loading = true;
    error   = '';
    try {
      const res = await fetch(`${baseApiUrl}/api/leaderboard`);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      entries = await res.json();
    } catch (e) {
      error = `Could not load rankings: ${e.message}`;
    } finally {
      loading = false;
    }
  }

  async function toggleRow(studentId) {
    if (expandedRow === studentId) { expandedRow = null; return; }
    expandedRow = studentId;
    if (activityCache[studentId] !== undefined || activityLoading[studentId]) return;

    activityLoading = { ...activityLoading, [studentId]: true };
    try {
      const res = await fetch(`${baseApiUrl}/api/users/${studentId}/activity`);
      activityCache = { ...activityCache, [studentId]: res.ok ? await res.json() : [] };
    } catch (_) {
      activityCache = { ...activityCache, [studentId]: [] };
    } finally {
      activityLoading = { ...activityLoading, [studentId]: false };
    }
  }

  // ── Heatmap utilities ─────────────────────────────────────────────────────

  function buildWeeks(activityData) {
    const map = {};
    for (const d of (activityData || [])) map[d.date] = d.count;

    const today = new Date();
    const cells = [];
    for (let i = 83; i >= 0; i--) {
      const d = new Date(today);
      d.setDate(d.getDate() - i);
      const y  = d.getFullYear();
      const mo = String(d.getMonth() + 1).padStart(2, '0');
      const dy = String(d.getDate()).padStart(2, '0');
      const ds = `${y}-${mo}-${dy}`;
      cells.push({ date: ds, count: map[ds] || 0 });
    }
    const weeks = [];
    for (let i = 0; i < cells.length; i += 7) weeks.push(cells.slice(i, i + 7));
    return weeks;
  }

  function maxCount(activityData) {
    if (!activityData || !activityData.length) return 1;
    return Math.max(...activityData.map(d => d.count), 1);
  }

  function heatBg(count, maxC) {
    if (!count) return 'rgba(255,255,255,0.05)';
    const t = Math.min(1, count / maxC);
    if (t < 0.25) return 'rgba(52,211,153,0.28)';
    if (t < 0.5)  return 'rgba(52,211,153,0.50)';
    if (t < 0.75) return 'rgba(52,211,153,0.74)';
    return 'rgba(52,211,153,1)';
  }

  function fmtDate(ds) {
    const d = new Date(ds + 'T00:00:00');
    return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
  }

  onMount(fetchLeaderboard);
</script>

<div class="lb-root">

  <!-- ── Personal Performance Banner ─────────────────────────────────────── -->
  {#if myEntry}
    {@const acc = accuracyPct(myEntry)}
    {@const tier = TIER[myEntry.rank_tier] ?? TIER['Novice Practitioner']}
    {@const pct = topPct(myEntry)}
    <div class="perf-banner">
      <!-- Rank block -->
      <div class="perf-block perf-rank-block">
        <span class="perf-kicker">Your Global Rank</span>
        <span class="perf-rank-num">#{myEntry.national_rank}</span>
        <span class="perf-rank-of">of {entries.length} candidates</span>
        <span class="perf-pct-pill">Top {pct}%</span>
      </div>

      <!-- Accuracy gauge -->
      <div class="perf-block perf-gauge-block">
        <span class="perf-kicker">Cumulative Accuracy</span>
        <svg class="gauge-svg" viewBox="0 0 100 100" width="110" height="110" role="img"
             aria-label="{myEntry.accuracy_total ? Math.round(acc * 100) : 'No data'}% accuracy">
          <!-- Background ring -->
          <circle cx="50" cy="50" r={GAUGE_R} fill="none"
                  stroke="rgba(255,255,255,0.07)" stroke-width="9"/>
          <!-- Progress arc -->
          {#if myEntry.accuracy_total > 0}
            <circle cx="50" cy="50" r={GAUGE_R} fill="none"
                    stroke={acc >= 0.75 ? '#34d399' : acc >= 0.5 ? '#f0c040' : '#fb7185'}
                    stroke-width="9"
                    stroke-dasharray="{(acc * GAUGE_C).toFixed(2)} {GAUGE_C.toFixed(2)}"
                    stroke-linecap="round"
                    transform="rotate(-90 50 50)"/>
          {/if}
          <!-- Center text -->
          {#if myEntry.accuracy_total > 0}
            <text x="50" y="47" text-anchor="middle" fill="#f0f0ff"
                  font-size="17" font-weight="700" font-family="monospace">
              {Math.round(acc * 100)}%
            </text>
            <text x="50" y="62" text-anchor="middle" fill="rgba(255,255,255,0.38)"
                  font-size="9.5" font-family="sans-serif">
              {myEntry.accuracy_correct}/{myEntry.accuracy_total}
            </text>
          {:else}
            <text x="50" y="53" text-anchor="middle" fill="rgba(255,255,255,0.3)"
                  font-size="10" font-family="sans-serif">No data yet</text>
          {/if}
        </svg>
      </div>

      <!-- Tier + rating block -->
      <div class="perf-block perf-tier-block">
        <span class="perf-kicker">Performance Tier</span>
        <span class="tier-badge {tier.cls} perf-tier-badge">
          <span class="tier-icon">{tier.icon}</span>
          {myEntry.rank_tier}
        </span>
        <span class="perf-rating-big">{myEntry.rating.toFixed(0)}</span>
        <span class="perf-rating-sub">Glicko-2 Rating · ±{myEntry.rating_deviation.toFixed(0)} RD</span>
        {#if myEntry.focus_badges && myEntry.focus_badges.length}
          <div class="perf-badges-row">
            {#each myEntry.focus_badges as badge}
              <span class="focus-badge" style="--bc: {BADGE_COLOR[badge] ?? '#888'}">{badge}</span>
            {/each}
          </div>
        {/if}
      </div>
    </div>
  {/if}

  <!-- ── Header ──────────────────────────────────────────────────────────── -->
  <div class="lb-head">
    <div class="lb-head-text">
      <h2 class="lb-title">National Skill Ranking</h2>
      <p class="lb-subtitle">Live Glicko-2 rating vectors &mdash; updated after each quiz submission</p>
    </div>
    <button class="lb-refresh-btn" onclick={fetchLeaderboard} disabled={loading}>
      {#if loading}<span class="spin"></span>{:else}🔄{/if}
      {loading ? 'Loading...' : 'Refresh'}
    </button>
  </div>

  <!-- ── Filter bar ─────────────────────────────────────────────────────── -->
  <div class="lb-filter-bar">
    <span class="filter-label">Display:</span>
    <div class="filter-group">
      <button class="filter-btn {!verifiedOnly ? 'active' : ''}" onclick={() => verifiedOnly = false}>
        All Users
      </button>
      <button class="filter-btn {verifiedOnly ? 'active' : ''}" onclick={() => verifiedOnly = true}>
        Verified Rankings
        <span class="filter-badge">RD &le; {RD_THRESHOLD}</span>
      </button>
    </div>
    {#if verifiedOnly}
      <span class="filter-note">
        Hiding {entries.length - displayed.length} unverified placement{entries.length - displayed.length !== 1 ? 's' : ''}
      </span>
    {/if}
  </div>

  <!-- ── States ─────────────────────────────────────────────────────────── -->
  {#if error}
    <div class="lb-error">{error}</div>

  {:else if loading}
    <div class="lb-skeleton" aria-label="Loading rankings">
      {#each { length: 6 } as _, i}
        <div class="sk-row" style="animation-delay: {i * 60}ms">
          <div class="sk-rank"></div>
          <div class="sk-avatar"></div>
          <div class="sk-text"></div>
          <div class="sk-badges"></div>
          <div class="sk-rating"></div>
        </div>
      {/each}
    </div>

  {:else if displayed.length === 0}
    <div class="lb-empty">
      {#if verifiedOnly}
        No verified rankings yet — confidence intervals are still expanding.
        <button class="lb-show-all-btn" onclick={() => verifiedOnly = false}>Show All Users</button>
      {:else}
        No ranked students yet. Ratings populate after the first quiz submission.
      {/if}
    </div>

  {:else}
    <div class="lb-table-wrap">
      <table class="lb-table">
        <thead>
          <tr>
            <th class="th-rank">Rank</th>
            <th class="th-avatar"></th>
            <th class="th-name">Candidate</th>
            <th class="th-badges">Focus Areas</th>
            <th class="th-rating">Glicko-2 Rating</th>
            <th class="th-conf">Confidence</th>
            <th class="th-games">Matches</th>
          </tr>
        </thead>

        {#each displayed as entry, i}
          {@const rank     = entry.national_rank ?? (i + 1)}
          {@const isPodium = rank <= 3}
          {@const tier     = TIER[entry.rank_tier] ?? TIER['Novice Practitioner']}
          {@const rd       = rdStatus(entry.rating_deviation)}
          {@const isMe     = auth.user && entry.student_id === auth.user.id}
          {@const isOpen   = expandedRow === entry.student_id}

          <!-- Glicko-2 interval visualization: scale [900, 2400] → 0–100% -->
          {@const R_MIN   = 900}
          {@const R_RANGE = 1500}
          {@const dotPct  = Math.max(0, Math.min(100, (entry.rating - R_MIN) / R_RANGE * 100))}
          {@const barLeft = Math.max(0, (entry.rating - entry.rating_deviation - R_MIN) / R_RANGE * 100)}
          {@const barW    = Math.min(100 - barLeft, (entry.rating_deviation * 2) / R_RANGE * 100)}

          <tbody class="lb-body {isOpen ? 'open' : ''}">
            <tr
              class="lb-row {isPodium ? `podium podium-${rank}` : ''} {isMe ? 'my-row' : ''} {isOpen ? 'row-open' : ''}"
              onclick={() => toggleRow(entry.student_id)}
              title="Click to expand activity heatmap"
            >
              <!-- Rank -->
              <td class="td-rank">
                {#if rank <= 3}
                  <span class="medal">{MEDALS[rank - 1]}</span>
                {:else}
                  <span class="rank-num">{rank}</span>
                {/if}
              </td>

              <!-- Avatar -->
              <td class="td-avatar">
                <div class="avatar" style="border-color: {tier.ring}; color: {tier.ring}">
                  {initials(entry.display_name)}
                </div>
              </td>

              <!-- Name + percentile -->
              <td class="td-name">
                <span class="candidate-name">
                  {entry.display_name}
                  {#if isMe}<span class="you-pill">you</span>{/if}
                </span>
                <span class="percentile-hint">Top {topPct(entry)}%</span>
              </td>

              <!-- Focus badges -->
              <td class="td-badges">
                {#if entry.focus_badges && entry.focus_badges.length}
                  <div class="badges-cell">
                    {#each entry.focus_badges.slice(0, 3) as badge}
                      <span class="focus-badge" style="--bc: {BADGE_COLOR[badge] ?? '#888'}">{badge}</span>
                    {/each}
                  </div>
                {:else}
                  <span class="no-badges">—</span>
                {/if}
              </td>

              <!-- Rating + RD interval bar -->
              <td class="td-rating">
                <span class="rating-num">{entry.rating.toFixed(0)}</span>
                <div class="interval-track" title="Rating ± {entry.rating_deviation.toFixed(0)} (95% interval)">
                  <div class="interval-range" style="left: {barLeft.toFixed(2)}%; width: {Math.max(2, barW).toFixed(2)}%"></div>
                  <div class="interval-dot" style="left: calc({dotPct.toFixed(2)}% - 3px)"></div>
                </div>
                <span class="rd-tag">±{entry.rating_deviation.toFixed(0)}</span>
              </td>

              <!-- Confidence status -->
              <td class="td-conf">
                <span class="rd-label {rd.cls}">{rd.label}</span>
              </td>

              <!-- Matches -->
              <td class="td-games">
                <span class="games-num">{entry.games_played}</span>
                <span class="expand-caret">{isOpen ? '▲' : '▼'}</span>
              </td>
            </tr>

            <!-- ── Activity Heatmap Drawer ─────────────────────────────── -->
            {#if isOpen}
              <tr transition:fade={{ duration: 160 }} class="drawer-row">
                <td colspan="7" class="drawer-cell">
                  {#if activityLoading[entry.student_id]}
                    <div class="drawer-loading">
                      <span class="spin"></span> Loading activity data…
                    </div>
                  {:else}
                    {@const ad   = activityCache[entry.student_id] || []}
                    {@const wks  = buildWeeks(ad)}
                    {@const maxC = maxCount(ad)}
                    <div class="drawer-inner">
                      <div class="drawer-header-row">
                        <span class="drawer-title">
                          12-Week Activity &mdash; {entry.display_name}
                        </span>
                        <span class="drawer-acc">
                          Accuracy:
                          {#if entry.accuracy_total > 0}
                            <strong>{Math.round(entry.accuracy_correct / entry.accuracy_total * 100)}%</strong>
                            <span class="drawer-acc-sub">({entry.accuracy_correct}/{entry.accuracy_total})</span>
                          {:else}
                            <span class="drawer-acc-empty">No quiz history yet</span>
                          {/if}
                        </span>
                      </div>

                      <div class="heatmap-wrap">
                        <!-- Day labels -->
                        <div class="heat-day-labels">
                          <span>Mon</span>
                          <span></span>
                          <span>Wed</span>
                          <span></span>
                          <span>Fri</span>
                          <span></span>
                          <span>Sun</span>
                        </div>
                        <!-- Grid -->
                        <div class="heatmap-grid">
                          {#each wks as week}
                            <div class="heat-week">
                              {#each week as cell}
                                <div
                                  class="heat-cell"
                                  style="background: {heatBg(cell.count, maxC)}"
                                  onmouseenter={() => hoveredCell = cell}
                                  onmouseleave={() => hoveredCell = null}
                                  role="img"
                                  aria-label="{fmtDate(cell.date)}: {cell.count} session{cell.count !== 1 ? 's' : ''}"
                                ></div>
                              {/each}
                            </div>
                          {/each}
                        </div>
                      </div>

                      <!-- Tooltip -->
                      {#if hoveredCell}
                        <div class="heat-tooltip" transition:fade={{ duration: 80 }}>
                          {fmtDate(hoveredCell.date)} &mdash;
                          {hoveredCell.count} quiz session{hoveredCell.count !== 1 ? 's' : ''}
                        </div>
                      {/if}

                      <!-- Legend -->
                      <div class="heat-legend">
                        <span>Less</span>
                        {#each [0, 1, 2, 3, 4] as lvl}
                          <div class="heat-legend-cell" style="background: {
                            ['rgba(255,255,255,0.05)','rgba(52,211,153,0.28)','rgba(52,211,153,0.50)','rgba(52,211,153,0.74)','rgba(52,211,153,1)'][lvl]
                          }"></div>
                        {/each}
                        <span>More</span>
                      </div>
                    </div>
                  {/if}
                </td>
              </tr>
            {/if}
          </tbody>
        {/each}
      </table>
    </div>
  {/if}
</div>

<style>
  /* ── Root ────────────────────────────────────────────────────────────── */
  .lb-root {
    width: 100%;
    background: rgba(255, 255, 255, 0.018);
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 16px;
    padding: 28px 32px;
    box-sizing: border-box;
  }

  /* ── Performance Banner ──────────────────────────────────────────────── */
  .perf-banner {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    gap: 1px;
    background: rgba(255, 255, 255, 0.07);
    border: 1px solid rgba(255, 255, 255, 0.09);
    border-radius: 14px;
    overflow: hidden;
    margin-bottom: 28px;
  }
  .perf-block {
    background: rgba(12, 14, 26, 0.55);
    padding: 20px 24px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
  }
  .perf-kicker {
    font-size: 0.68rem;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.35);
    margin-bottom: 4px;
  }

  /* Rank block */
  .perf-rank-num {
    font-size: 2.8rem;
    font-weight: 800;
    color: #f0f0ff;
    line-height: 1;
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.03em;
  }
  .perf-rank-of {
    font-size: 0.78rem;
    color: rgba(255, 255, 255, 0.4);
  }
  .perf-pct-pill {
    display: inline-block;
    background: rgba(240, 192, 64, 0.14);
    border: 1px solid rgba(240, 192, 64, 0.3);
    color: #f0c040;
    border-radius: 20px;
    padding: 3px 12px;
    font-size: 0.75rem;
    font-weight: 700;
    margin-top: 4px;
  }

  /* Gauge block */
  .perf-gauge-block { justify-content: center; gap: 8px; }
  .gauge-svg { display: block; }

  /* Tier block */
  .perf-tier-block { align-items: center; }
  .perf-tier-badge { font-size: 0.82rem !important; }
  .perf-rating-big {
    font-size: 2rem;
    font-weight: 800;
    color: #f0f0ff;
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.02em;
  }
  .perf-rating-sub {
    font-size: 0.72rem;
    color: rgba(255, 255, 255, 0.35);
  }
  .perf-badges-row {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    justify-content: center;
    margin-top: 6px;
  }

  /* ── Section header ──────────────────────────────────────────────────── */
  .lb-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 18px;
  }
  .lb-title {
    font-size: 1.3rem;
    font-weight: 700;
    color: #f0f0ff;
    margin: 0 0 3px;
    letter-spacing: -0.01em;
  }
  .lb-subtitle { font-size: 0.8rem; color: rgba(255, 255, 255, 0.4); margin: 0; }
  .lb-refresh-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.7);
    border-radius: 8px;
    padding: 7px 14px;
    font-size: 0.8rem;
    cursor: pointer;
    white-space: nowrap;
    flex-shrink: 0;
    transition: background 0.15s, color 0.15s;
  }
  .lb-refresh-btn:hover:not(:disabled) { background: rgba(255,255,255,0.1); color: #fff; }
  .lb-refresh-btn:disabled { opacity: 0.45; cursor: not-allowed; }

  /* ── Filter bar ──────────────────────────────────────────────────────── */
  .lb-filter-bar {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 20px;
    flex-wrap: wrap;
  }
  .filter-label { font-size: 0.8rem; color: rgba(255,255,255,0.4); white-space: nowrap; }
  .filter-group { display: flex; gap: 6px; }
  .filter-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    background: rgba(255,255,255,0.05);
    border: 1px solid rgba(255,255,255,0.09);
    color: rgba(255,255,255,0.55);
    border-radius: 8px;
    padding: 6px 12px;
    font-size: 0.8rem;
    cursor: pointer;
    transition: all 0.15s;
  }
  .filter-btn:hover { background: rgba(255,255,255,0.09); color: #fff; }
  .filter-btn.active {
    background: rgba(240,192,64,0.12);
    border-color: rgba(240,192,64,0.35);
    color: #f0c040;
  }
  .filter-badge {
    background: rgba(240,192,64,0.15);
    color: #f0c040;
    border-radius: 4px;
    padding: 1px 5px;
    font-size: 0.7rem;
    font-family: monospace;
  }
  .filter-note { font-size: 0.75rem; color: rgba(255,255,255,0.35); font-style: italic; }

  /* ── Error / empty ────────────────────────────────────────────────────── */
  .lb-error {
    padding: 20px;
    background: rgba(255,80,80,0.08);
    border: 1px solid rgba(255,80,80,0.2);
    border-radius: 10px;
    color: #ff8080;
    font-size: 0.85rem;
    text-align: center;
  }
  .lb-empty {
    padding: 48px 24px;
    text-align: center;
    color: rgba(255,255,255,0.4);
    font-size: 0.9rem;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
  }
  .lb-show-all-btn {
    background: rgba(255,255,255,0.06);
    border: 1px solid rgba(255,255,255,0.1);
    color: rgba(255,255,255,0.7);
    border-radius: 8px;
    padding: 8px 16px;
    font-size: 0.8rem;
    cursor: pointer;
    transition: all 0.15s;
  }
  .lb-show-all-btn:hover { background: rgba(255,255,255,0.1); color: #fff; }

  /* ── Loading skeleton ────────────────────────────────────────────────── */
  .lb-skeleton { display: flex; flex-direction: column; gap: 10px; }
  .sk-row {
    display: grid;
    grid-template-columns: 48px 40px 1fr 160px 80px;
    gap: 12px;
    align-items: center;
    padding: 10px 4px;
    border-radius: 8px;
    animation: sk-fade 1.4s ease-in-out infinite;
  }
  .sk-rank, .sk-avatar, .sk-text, .sk-badges, .sk-rating {
    background: rgba(255,255,255,0.06);
    border-radius: 6px;
    height: 18px;
  }
  .sk-rank   { width: 32px; }
  .sk-avatar { width: 36px; height: 36px; border-radius: 50%; }
  .sk-text   { height: 14px; }
  .sk-badges { height: 22px; border-radius: 20px; }
  .sk-rating { width: 56px; }
  @keyframes sk-fade { 0%, 100% { opacity: 0.45; } 50% { opacity: 0.12; } }

  /* ── Table ────────────────────────────────────────────────────────────── */
  .lb-table-wrap { overflow-x: auto; }
  .lb-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.84rem;
  }
  .lb-table th {
    padding: 9px 12px;
    text-align: left;
    font-size: 0.7rem;
    font-weight: 600;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: rgba(255,255,255,0.3);
    border-bottom: 1px solid rgba(255,255,255,0.07);
    white-space: nowrap;
  }
  .th-rating, .th-conf, .th-games { text-align: right; }

  /* Tbody grouping per row — ensures drawer row stays adjacent */
  .lb-body { display: table-row-group; }

  .lb-row {
    border-bottom: 1px solid rgba(255,255,255,0.04);
    cursor: pointer;
    transition: background 0.12s;
  }
  .lb-row:hover { background: rgba(255,255,255,0.028); }
  .lb-row td { padding: 11px 12px; vertical-align: middle; }

  /* Podium highlight */
  .podium-1 { background: rgba(255,196,0,0.055); }
  .podium-2 { background: rgba(192,192,192,0.04); }
  .podium-3 { background: rgba(180,120,60,0.04); }
  .podium-1:hover { background: rgba(255,196,0,0.09); }
  .podium-2:hover { background: rgba(192,192,192,0.07); }
  .podium-3:hover { background: rgba(180,120,60,0.07); }

  /* Current user highlight */
  .my-row { background: rgba(96,165,250,0.05) !important; }
  .my-row:hover { background: rgba(96,165,250,0.09) !important; }

  /* Open row */
  .row-open { border-bottom: none; }

  /* Rank */
  .td-rank { width: 52px; }
  .medal { font-size: 1.25rem; line-height: 1; }
  .rank-num {
    display: inline-block;
    width: 28px;
    text-align: center;
    font-size: 0.8rem;
    font-weight: 600;
    color: rgba(255,255,255,0.38);
  }

  /* Avatar */
  .td-avatar { width: 44px; }
  .avatar {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    background: rgba(255,255,255,0.06);
    border: 2px solid;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.71rem;
    font-weight: 700;
    letter-spacing: 0.03em;
    user-select: none;
  }

  /* Name */
  .td-name { min-width: 130px; }
  .candidate-name {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
    color: #e8e8f0;
    font-size: 0.87rem;
  }
  .you-pill {
    font-size: 0.62rem;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    background: rgba(96,165,250,0.2);
    color: #60a5fa;
    border-radius: 4px;
    padding: 1px 5px;
  }
  .percentile-hint { font-size: 0.7rem; color: rgba(255,255,255,0.32); display: block; margin-top: 2px; }

  /* Focus badges */
  .td-badges { min-width: 160px; }
  .badges-cell { display: flex; flex-wrap: wrap; gap: 4px; }
  .focus-badge {
    display: inline-block;
    background: color-mix(in srgb, var(--bc) 16%, transparent);
    border: 1px solid color-mix(in srgb, var(--bc) 40%, transparent);
    color: var(--bc);
    border-radius: 4px;
    padding: 2px 7px;
    font-size: 0.68rem;
    font-weight: 600;
    white-space: nowrap;
  }
  .no-badges { color: rgba(255,255,255,0.2); font-size: 0.8rem; }

  /* Rating + interval */
  .td-rating { text-align: right; min-width: 140px; }
  .rating-num {
    font-size: 1rem;
    font-weight: 700;
    color: #f0f0ff;
    font-variant-numeric: tabular-nums;
    display: block;
    text-align: right;
  }
  .interval-track {
    position: relative;
    width: 100%;
    height: 4px;
    background: rgba(255,255,255,0.07);
    border-radius: 2px;
    margin: 5px 0 3px;
    overflow: visible;
  }
  .interval-range {
    position: absolute;
    height: 100%;
    background: rgba(96,165,250,0.28);
    border-radius: 2px;
    top: 0;
  }
  .interval-dot {
    position: absolute;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #60a5fa;
    top: -1px;
    box-shadow: 0 0 4px rgba(96,165,250,0.6);
  }
  .rd-tag {
    font-size: 0.68rem;
    color: rgba(255,255,255,0.3);
    font-variant-numeric: tabular-nums;
    display: block;
    text-align: right;
  }

  /* Confidence */
  .td-conf { text-align: right; min-width: 90px; }
  .rd-label {
    font-size: 0.68rem;
    font-weight: 600;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }
  .rd-label.rd-verified   { color: #34d399; }
  .rd-label.rd-active     { color: #f0c040; }
  .rd-label.rd-provisional { color: rgba(255,255,255,0.28); }

  /* Matches + caret */
  .td-games { text-align: right; white-space: nowrap; }
  .games-num { font-size: 0.85rem; color: rgba(255,255,255,0.5); font-variant-numeric: tabular-nums; }
  .expand-caret { font-size: 0.6rem; color: rgba(255,255,255,0.2); margin-left: 6px; vertical-align: middle; }

  /* Tier badges */
  .tier-badge {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    border-radius: 20px;
    padding: 4px 11px;
    font-size: 0.76rem;
    font-weight: 600;
    white-space: nowrap;
  }
  .tier-icon { font-size: 0.7rem; }
  .tier-master   { background: rgba(255,196,0,0.12);   color: #ffc400; border: 1px solid rgba(255,196,0,0.3); }
  .tier-advanced { background: rgba(96,165,250,0.12);  color: #60a5fa; border: 1px solid rgba(96,165,250,0.3); }
  .tier-senior   { background: rgba(52,211,153,0.12);  color: #34d399; border: 1px solid rgba(52,211,153,0.3); }
  .tier-novice   { background: rgba(255,255,255,0.05); color: rgba(255,255,255,0.4); border: 1px solid rgba(255,255,255,0.09); }

  /* ── Activity Heatmap Drawer ─────────────────────────────────────────── */
  .drawer-row { background: rgba(255,255,255,0.012); }
  .drawer-cell {
    padding: 0 !important;
    border-bottom: 1px solid rgba(255,255,255,0.06);
  }
  .drawer-inner {
    padding: 18px 20px 22px;
  }
  .drawer-loading {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 20px;
    font-size: 0.82rem;
    color: rgba(255,255,255,0.4);
  }
  .drawer-header-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 14px;
    gap: 16px;
    flex-wrap: wrap;
  }
  .drawer-title {
    font-size: 0.8rem;
    font-weight: 600;
    color: rgba(255,255,255,0.55);
    letter-spacing: 0.02em;
  }
  .drawer-acc {
    font-size: 0.8rem;
    color: rgba(255,255,255,0.45);
  }
  .drawer-acc strong { color: #f0f0ff; font-weight: 700; }
  .drawer-acc-sub { color: rgba(255,255,255,0.3); margin-left: 4px; }
  .drawer-acc-empty { color: rgba(255,255,255,0.25); }

  /* Heatmap */
  .heatmap-wrap {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    overflow-x: auto;
    padding-bottom: 4px;
  }
  .heat-day-labels {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding-top: 1px;
    min-width: 28px;
  }
  .heat-day-labels span {
    font-size: 0.58rem;
    color: rgba(255,255,255,0.25);
    height: 12px;
    line-height: 12px;
    white-space: nowrap;
  }
  .heatmap-grid { display: flex; gap: 3px; }
  .heat-week { display: flex; flex-direction: column; gap: 3px; }
  .heat-cell {
    width: 12px;
    height: 12px;
    border-radius: 2px;
    transition: transform 0.1s, filter 0.1s;
    cursor: crosshair;
    flex-shrink: 0;
  }
  .heat-cell:hover {
    transform: scale(1.4);
    filter: brightness(1.3);
    z-index: 10;
    position: relative;
  }

  .heat-tooltip {
    font-size: 0.72rem;
    color: rgba(255,255,255,0.6);
    margin-top: 8px;
    padding: 4px 8px;
    background: rgba(0,0,0,0.4);
    border-radius: 5px;
    display: inline-block;
    border: 1px solid rgba(255,255,255,0.08);
  }

  .heat-legend {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-top: 10px;
    font-size: 0.65rem;
    color: rgba(255,255,255,0.3);
  }
  .heat-legend-cell {
    width: 11px;
    height: 11px;
    border-radius: 2px;
  }

  /* ── Spinner ─────────────────────────────────────────────────────────── */
  .spin {
    display: inline-block;
    width: 12px;
    height: 12px;
    border: 2px solid rgba(255,255,255,0.18);
    border-top-color: rgba(255,255,255,0.7);
    border-radius: 50%;
    animation: lb-spin 0.7s linear infinite;
    flex-shrink: 0;
  }
  @keyframes lb-spin { to { transform: rotate(360deg); } }

  /* ── Responsive ──────────────────────────────────────────────────────── */
  @media (max-width: 900px) {
    .perf-banner { grid-template-columns: 1fr 1fr; }
    .perf-gauge-block { grid-column: span 2; border-top: 1px solid rgba(255,255,255,0.07); }
    .th-conf, .td-conf { display: none; }
  }
  @media (max-width: 680px) {
    .lb-root { padding: 16px 12px; }
    .perf-banner { grid-template-columns: 1fr; }
    .perf-gauge-block { grid-column: span 1; }
    .th-badges, .td-badges { display: none; }
    .th-games, .td-games { display: none; }
  }
</style>
