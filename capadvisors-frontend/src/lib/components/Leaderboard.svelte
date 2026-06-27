<script>
  import { onMount } from 'svelte';

  let { baseApiUrl = '' } = $props();

  let entries = $state([]);
  let loading = $state(true);
  let error = $state('');
  let verifiedOnly = $state(false);

  const RD_THRESHOLD = 80;
  const MEDALS = ['🥇', '🥈', '🥉'];

  const TIER = {
    'Strategic Master':    { cls: 'tier-master',   icon: '♛' },
    'Advanced Analyst':    { cls: 'tier-advanced',  icon: '◆' },
    'Senior Candidate':    { cls: 'tier-senior',    icon: '●' },
    'Novice Practitioner': { cls: 'tier-novice',    icon: '○' },
  };

  let displayed = $derived(
    verifiedOnly ? entries.filter(e => e.rating_deviation <= RD_THRESHOLD) : entries
  );

  function initials(name) {
    if (!name) return '??';
    return name.trim().split(/\s+/).map(w => w[0] ?? '').join('').toUpperCase().slice(0, 2);
  }

  function rdStatus(rd) {
    if (rd <= 80)  return { label: 'Verified',    cls: 'rd-verified' };
    if (rd <= 150) return { label: 'Active',       cls: 'rd-active' };
    return             { label: 'Provisional',  cls: 'rd-provisional' };
  }

  async function fetchLeaderboard() {
    loading = true;
    error = '';
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

  onMount(fetchLeaderboard);
</script>

<div class="lb-root">
  <!-- Header -->
  <div class="lb-head">
    <div class="lb-head-text">
      <h2 class="lb-title">National Skill Ranking</h2>
      <p class="lb-subtitle">Live Glicko-2 rating vectors &mdash; updated after each tournament</p>
    </div>
    <button class="lb-refresh-btn" onclick={fetchLeaderboard} disabled={loading}>
      {#if loading}<span class="spin"></span>{:else}🔄{/if}
      {loading ? 'Loading...' : 'Refresh'}
    </button>
  </div>

  <!-- Filter toggle -->
  <div class="lb-filter-bar">
    <span class="filter-label">Display:</span>
    <div class="filter-group">
      <button
        class="filter-btn {!verifiedOnly ? 'active' : ''}"
        onclick={() => verifiedOnly = false}
      >All Users</button>
      <button
        class="filter-btn {verifiedOnly ? 'active' : ''}"
        onclick={() => verifiedOnly = true}
      >Verified Rankings Only
        <span class="filter-badge">RD &le; {RD_THRESHOLD}</span>
      </button>
    </div>
    {#if verifiedOnly}
      <span class="filter-note">
        Hiding {entries.length - displayed.length} unverified placement{entries.length - displayed.length !== 1 ? 's' : ''}
      </span>
    {/if}
  </div>

  <!-- States -->
  {#if error}
    <div class="lb-error">{error}</div>
  {:else if loading}
    <div class="lb-skeleton" aria-label="Loading rankings">
      {#each { length: 6 } as _, i}
        <div class="sk-row" style="animation-delay: {i * 60}ms">
          <div class="sk-rank"></div>
          <div class="sk-avatar"></div>
          <div class="sk-text"></div>
          <div class="sk-tier"></div>
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
        No ranked students yet. Ratings populate after the first tournament.
      {/if}
    </div>
  {:else}
    <div class="lb-table-wrap">
      <table class="lb-table">
        <thead>
          <tr>
            <th class="th-rank">Rank</th>
            <th class="th-candidate" colspan="2">Candidate</th>
            <th class="th-tier">Performance Tier</th>
            <th class="th-rating">Skill Rating</th>
            <th class="th-rd">Confidence</th>
            <th class="th-games">Matches</th>
          </tr>
        </thead>
        <tbody>
          {#each displayed as entry, i}
            {@const rank = entry.national_rank ?? (i + 1)}
            {@const isPodium = rank <= 3}
            {@const tier = TIER[entry.rank_tier] ?? { cls: 'tier-novice', icon: '○' }}
            {@const rd = rdStatus(entry.rating_deviation)}
            <tr class="lb-row {isPodium ? `podium podium-${rank}` : ''}">
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
                <div class="avatar {isPodium ? `avatar-podium-${rank}` : ''}">
                  {initials(entry.display_name)}
                </div>
              </td>

              <!-- Name -->
              <td class="td-name">
                <span class="candidate-name">{entry.display_name}</span>
                <span class="percentile-hint">
                  Top {Math.max(1, Math.ceil((rank / entries.length) * 100))}%
                </span>
              </td>

              <!-- Tier badge -->
              <td class="td-tier">
                <span class="tier-badge {tier.cls}">
                  <span class="tier-icon">{tier.icon}</span>
                  {entry.rank_tier}
                </span>
              </td>

              <!-- Rating -->
              <td class="td-rating">
                <span class="rating-num">{entry.rating.toFixed(0)}</span>
              </td>

              <!-- RD confidence -->
              <td class="td-rd">
                <div class="rd-wrap">
                  <div class="rd-bar-track">
                    <div
                      class="rd-bar-fill {rd.cls}"
                      style="width: {Math.min(100, Math.round((entry.rating_deviation / 350) * 100))}%"
                    ></div>
                  </div>
                  <span class="rd-label {rd.cls}">{rd.label}</span>
                </div>
              </td>

              <!-- Matches played -->
              <td class="td-games">
                <span class="games-num">{entry.games_played}</span>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<style>
  .lb-root {
    width: 100%;
    background: rgba(255, 255, 255, 0.025);
    border: 1px solid rgba(255, 255, 255, 0.07);
    border-radius: 16px;
    padding: 28px 32px;
    box-sizing: border-box;
  }

  /* ── Header ─────────────────────────────────────────── */
  .lb-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 20px;
  }
  .lb-title {
    font-size: 1.35rem;
    font-weight: 700;
    color: #f0f0ff;
    margin: 0 0 4px;
    letter-spacing: -0.01em;
  }
  .lb-subtitle {
    font-size: 0.82rem;
    color: rgba(255, 255, 255, 0.45);
    margin: 0;
  }
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
    transition: background 0.15s, color 0.15s;
    flex-shrink: 0;
  }
  .lb-refresh-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
  }
  .lb-refresh-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  /* ── Filter bar ──────────────────────────────────────── */
  .lb-filter-bar {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 20px;
    flex-wrap: wrap;
  }
  .filter-label {
    font-size: 0.8rem;
    color: rgba(255, 255, 255, 0.4);
    white-space: nowrap;
  }
  .filter-group {
    display: flex;
    gap: 6px;
  }
  .filter-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.09);
    color: rgba(255, 255, 255, 0.55);
    border-radius: 8px;
    padding: 6px 12px;
    font-size: 0.8rem;
    cursor: pointer;
    transition: all 0.15s;
  }
  .filter-btn:hover { background: rgba(255, 255, 255, 0.09); color: #fff; }
  .filter-btn.active {
    background: rgba(240, 192, 64, 0.12);
    border-color: rgba(240, 192, 64, 0.35);
    color: #f0c040;
  }
  .filter-badge {
    background: rgba(240, 192, 64, 0.15);
    color: #f0c040;
    border-radius: 4px;
    padding: 1px 5px;
    font-size: 0.7rem;
    font-family: monospace;
  }
  .filter-note {
    font-size: 0.75rem;
    color: rgba(255, 255, 255, 0.35);
    font-style: italic;
  }

  /* ── Error / empty ───────────────────────────────────── */
  .lb-error {
    padding: 20px;
    background: rgba(255, 80, 80, 0.08);
    border: 1px solid rgba(255, 80, 80, 0.2);
    border-radius: 10px;
    color: #ff8080;
    font-size: 0.85rem;
    text-align: center;
  }
  .lb-empty {
    padding: 48px 24px;
    text-align: center;
    color: rgba(255, 255, 255, 0.4);
    font-size: 0.9rem;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
  }
  .lb-show-all-btn {
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.7);
    border-radius: 8px;
    padding: 8px 16px;
    font-size: 0.8rem;
    cursor: pointer;
    transition: all 0.15s;
  }
  .lb-show-all-btn:hover { background: rgba(255, 255, 255, 0.1); color: #fff; }

  /* ── Loading skeleton ────────────────────────────────── */
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
  .sk-rank, .sk-avatar, .sk-text, .sk-tier, .sk-rating {
    background: rgba(255, 255, 255, 0.06);
    border-radius: 6px;
    height: 18px;
  }
  .sk-rank  { width: 32px; }
  .sk-avatar { width: 36px; height: 36px; border-radius: 50%; }
  .sk-text  { height: 14px; }
  .sk-tier  { height: 22px; width: 130px; border-radius: 20px; }
  .sk-rating { width: 56px; }
  @keyframes sk-fade {
    0%, 100% { opacity: 0.45; }
    50%       { opacity: 0.15; }
  }

  /* ── Table ───────────────────────────────────────────── */
  .lb-table-wrap { overflow-x: auto; }
  .lb-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.85rem;
  }
  .lb-table th {
    padding: 10px 12px;
    text-align: left;
    font-size: 0.72rem;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.35);
    border-bottom: 1px solid rgba(255, 255, 255, 0.07);
    white-space: nowrap;
  }
  .th-rating, .th-rd, .th-games { text-align: right; }

  .lb-row {
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
    transition: background 0.12s;
  }
  .lb-row:hover { background: rgba(255, 255, 255, 0.025); }
  .lb-row td { padding: 11px 12px; vertical-align: middle; }

  /* Podium rows */
  .podium-1 { background: rgba(255, 196, 0, 0.06); }
  .podium-2 { background: rgba(192, 192, 192, 0.05); }
  .podium-3 { background: rgba(180, 120, 60, 0.05); }
  .podium-1:hover { background: rgba(255, 196, 0, 0.1); }
  .podium-2:hover { background: rgba(192, 192, 192, 0.08); }
  .podium-3:hover { background: rgba(180, 120, 60, 0.08); }

  /* Rank column */
  .td-rank { width: 52px; }
  .medal { font-size: 1.3rem; line-height: 1; }
  .rank-num {
    display: inline-block;
    width: 28px;
    text-align: center;
    font-size: 0.8rem;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.4);
  }

  /* Avatar */
  .td-avatar { width: 44px; }
  .avatar {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.1);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.72rem;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.7);
    letter-spacing: 0.03em;
    user-select: none;
  }
  .avatar-podium-1 {
    border-color: rgba(255, 196, 0, 0.6);
    background: rgba(255, 196, 0, 0.1);
    color: #ffc400;
  }
  .avatar-podium-2 {
    border-color: rgba(192, 192, 192, 0.5);
    background: rgba(192, 192, 192, 0.08);
    color: #c0c0c0;
  }
  .avatar-podium-3 {
    border-color: rgba(205, 127, 50, 0.5);
    background: rgba(205, 127, 50, 0.08);
    color: #cd7f32;
  }

  /* Name */
  .td-name { min-width: 140px; }
  .candidate-name {
    display: block;
    font-weight: 600;
    color: #e8e8f0;
    font-size: 0.88rem;
  }
  .percentile-hint {
    font-size: 0.71rem;
    color: rgba(255, 255, 255, 0.35);
  }

  /* Tier badge */
  .td-tier { min-width: 170px; }
  .tier-badge {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    border-radius: 20px;
    padding: 4px 10px;
    font-size: 0.76rem;
    font-weight: 600;
    white-space: nowrap;
  }
  .tier-icon { font-size: 0.7rem; }
  .tier-master  { background: rgba(255, 196, 0, 0.12); color: #ffc400; border: 1px solid rgba(255, 196, 0, 0.3); }
  .tier-advanced { background: rgba(96, 165, 250, 0.12); color: #60a5fa; border: 1px solid rgba(96, 165, 250, 0.3); }
  .tier-senior  { background: rgba(52, 211, 153, 0.12); color: #34d399; border: 1px solid rgba(52, 211, 153, 0.3); }
  .tier-novice  { background: rgba(255, 255, 255, 0.05); color: rgba(255, 255, 255, 0.45); border: 1px solid rgba(255, 255, 255, 0.1); }

  /* Rating */
  .td-rating { text-align: right; }
  .rating-num {
    font-size: 1rem;
    font-weight: 700;
    color: #f0f0ff;
    font-variant-numeric: tabular-nums;
  }

  /* RD confidence */
  .td-rd { text-align: right; min-width: 110px; }
  .rd-wrap { display: flex; flex-direction: column; align-items: flex-end; gap: 3px; }
  .rd-bar-track {
    width: 72px;
    height: 4px;
    background: rgba(255, 255, 255, 0.08);
    border-radius: 2px;
    overflow: hidden;
  }
  .rd-bar-fill {
    height: 100%;
    border-radius: 2px;
    transition: width 0.4s ease;
  }
  .rd-verified  .rd-bar-fill, .rd-bar-fill.rd-verified  { background: #34d399; }
  .rd-active    .rd-bar-fill, .rd-bar-fill.rd-active    { background: #f0c040; }
  .rd-provisional .rd-bar-fill, .rd-bar-fill.rd-provisional { background: rgba(255, 255, 255, 0.25); }
  .rd-label {
    font-size: 0.68rem;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
  .rd-label.rd-verified   { color: #34d399; }
  .rd-label.rd-active     { color: #f0c040; }
  .rd-label.rd-provisional { color: rgba(255, 255, 255, 0.3); }

  /* Games */
  .td-games { text-align: right; }
  .games-num { font-size: 0.85rem; color: rgba(255, 255, 255, 0.55); font-variant-numeric: tabular-nums; }

  /* Spinner (inline) */
  .spin {
    display: inline-block;
    width: 12px;
    height: 12px;
    border: 2px solid rgba(255, 255, 255, 0.2);
    border-top-color: rgba(255, 255, 255, 0.7);
    border-radius: 50%;
    animation: lb-spin 0.7s linear infinite;
  }
  @keyframes lb-spin { to { transform: rotate(360deg); } }

  @media (max-width: 700px) {
    .lb-root { padding: 18px 14px; }
    .th-rd, .td-rd, .th-games, .td-games { display: none; }
  }
</style>
