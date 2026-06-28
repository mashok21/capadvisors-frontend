<script>
  import { onMount } from 'svelte';
  import { auth } from '../auth.svelte.js';

  let { baseApiUrl } = $props();

  const FIRM_GROUPS = [
    {
      label: 'Big 4',
      options: ['Deloitte', 'KPMG', 'EY', 'PwC'],
    },
    {
      label: 'Mid-Firms & Other',
      options: ['Top-20 Mid-Firm', 'None / Other'],
    },
  ];

  const YEARS = ['1st Year', '2nd Year', 'Completed', 'Direct Entry'];

  // ── State ──────────────────────────────────────────────────────────────────
  let isLoading        = $state(true);
  let isSaving         = $state(false);
  let isUploadingAvatar = $state(false);

  let avatarUrl        = $state('');
  let firm             = $state('None / Other');
  let year             = $state('1st Year');
  let location         = $state('');

  // Snapshot for dirty detection
  let savedFirm        = $state('None / Other');
  let savedYear        = $state('1st Year');
  let savedLocation    = $state('');

  let flash = $state(/** @type {{ kind: 'ok'|'err', msg: string }|null} */ (null));
  let flashTimerId = null;

  // ── Derived ────────────────────────────────────────────────────────────────
  let isDirty = $derived(
    firm !== savedFirm || year !== savedYear || location !== savedLocation
  );

  let initials = $derived(
    auth.user?.email
      ? auth.user.email.substring(0, 2).toUpperCase()
      : 'CA'
  );

  // ── API helpers ────────────────────────────────────────────────────────────
  function bearerHeaders(json = true) {
    const h = { Authorization: `Bearer ${auth.token}` };
    if (json) h['Content-Type'] = 'application/json';
    return h;
  }

  async function loadProfile() {
    isLoading = true;
    try {
      const res = await fetch(`${baseApiUrl}/api/user/profile`, {
        headers: bearerHeaders(false),
      });
      if (!res.ok) throw new Error(await res.text());
      const d = await res.json();
      avatarUrl    = d.avatar_url    || '';
      firm         = d.articleship_firm || 'None / Other';
      year         = d.articleship_year || '1st Year';
      location     = d.firm_location || '';
      savedFirm    = firm;
      savedYear    = year;
      savedLocation = location;
    } catch (e) {
      showFlash('err', `Could not load profile: ${e.message}`);
    } finally {
      isLoading = false;
    }
  }

  async function saveProfile() {
    isSaving = true;
    try {
      const res = await fetch(`${baseApiUrl}/api/user/profile`, {
        method: 'PUT',
        headers: bearerHeaders(),
        body: JSON.stringify({
          articleship_firm: firm,
          articleship_year: year,
          firm_location:    location,
        }),
      });
      if (!res.ok) throw new Error(await res.text());
      const d = await res.json();
      savedFirm     = d.articleship_firm;
      savedYear     = d.articleship_year;
      savedLocation = d.firm_location;
      showFlash('ok', 'Profile saved successfully.');
    } catch (e) {
      showFlash('err', `Save failed: ${e.message}`);
    } finally {
      isSaving = false;
    }
  }

  async function handleAvatarSelect(e) {
    const file = e.target.files?.[0];
    if (!file) return;
    isUploadingAvatar = true;
    try {
      const form = new FormData();
      form.append('avatar', file);
      const res = await fetch(`${baseApiUrl}/api/user/avatar/upload`, {
        method: 'POST',
        headers: { Authorization: `Bearer ${auth.token}` },
        body: form,
      });
      if (!res.ok) throw new Error(await res.text());
      const d = await res.json();
      avatarUrl = d.avatar_url;
      showFlash('ok', 'Avatar updated.');
    } catch (e) {
      showFlash('err', `Avatar upload failed: ${e.message}`);
    } finally {
      isUploadingAvatar = false;
      e.target.value = '';
    }
  }

  function showFlash(kind, msg) {
    clearTimeout(flashTimerId);
    flash = { kind, msg };
    flashTimerId = setTimeout(() => { flash = null; }, 4500);
  }

  onMount(loadProfile);
</script>

<div class="ep-root">

  <!-- Flash message -->
  {#if flash}
    <div class="ep-flash ep-flash--{flash.kind}" role="alert">
      {flash.kind === 'ok' ? '✓' : '✕'} {flash.msg}
    </div>
  {/if}

  {#if isLoading}
    <div class="ep-loading-state">
      <span class="ep-spinner ep-spinner--lg"></span>
      <p>Loading profile…</p>
    </div>
  {:else}
    <div class="ep-card">

      <!-- Avatar section -->
      <div class="ep-avatar-section">
        <label class="ep-avatar-btn" title="Click to upload a new photo">
          {#if isUploadingAvatar}
            <span class="ep-spinner ep-spinner--avatar"></span>
          {:else if avatarUrl}
            <img src={avatarUrl} alt="Your avatar" class="ep-avatar-img" />
          {:else}
            <div class="ep-avatar-placeholder">{initials}</div>
          {/if}
          <div class="ep-avatar-overlay">
            <span class="ep-camera-icon">📷</span>
          </div>
          <input
            type="file"
            accept="image/jpeg,image/png,image/webp,image/gif"
            class="ep-hidden-input"
            onchange={handleAvatarSelect}
            disabled={isUploadingAvatar}
          />
        </label>
        <div class="ep-avatar-meta">
          <p class="ep-avatar-label">Profile Photo</p>
          <p class="ep-avatar-hint">JPEG · PNG · WebP · GIF · max 2 MB</p>
        </div>
      </div>

      <!-- Form fields -->
      <div class="ep-fields">

        <!-- Articleship Firm -->
        <div class="ep-field-group">
          <label class="ep-label" for="ep-firm">Articleship Firm</label>
          <select id="ep-firm" class="ep-select" bind:value={firm}>
            {#each FIRM_GROUPS as group}
              <optgroup label={group.label}>
                {#each group.options as opt}
                  <option value={opt}>{opt}</option>
                {/each}
              </optgroup>
            {/each}
          </select>
        </div>

        <!-- Training Year -->
        <div class="ep-field-group">
          <label class="ep-label" for="ep-year">Articleship Year</label>
          <select id="ep-year" class="ep-select" bind:value={year}>
            {#each YEARS as y}
              <option value={y}>{y}</option>
            {/each}
          </select>
        </div>

        <!-- Firm Location -->
        <div class="ep-field-group">
          <label class="ep-label" for="ep-location">Firm Location</label>
          <input
            id="ep-location"
            type="text"
            class="ep-input"
            placeholder="e.g. Mumbai, Delhi, Bengaluru"
            bind:value={location}
            maxlength="100"
          />
        </div>

      </div>

      <!-- Action strip -->
      <div class="ep-actions">
        <button
          class="ep-save-btn"
          onclick={saveProfile}
          disabled={isSaving || !isDirty}
        >
          {#if isSaving}
            <span class="ep-spinner ep-spinner--sm"></span>
            Saving…
          {:else}
            Save Profile
          {/if}
        </button>
        {#if isDirty}
          <button
            class="ep-discard-btn"
            onclick={() => { firm = savedFirm; year = savedYear; location = savedLocation; }}
          >
            Discard Changes
          </button>
        {/if}
      </div>

    </div>
  {/if}

</div>

<style>
  .ep-root {
    max-width: 560px;
    margin: 32px auto;
    padding: 0 16px;
    font-family: inherit;
  }

  /* Flash */
  .ep-flash {
    margin-bottom: 18px;
    padding: 12px 16px;
    border-radius: 8px;
    font-size: 0.875rem;
    font-weight: 500;
    animation: ep-slide-in 0.2s ease;
  }
  .ep-flash--ok  { background: #d1fae5; color: #065f46; border: 1px solid #6ee7b7; }
  .ep-flash--err { background: #fee2e2; color: #991b1b; border: 1px solid #fca5a5; }

  /* Loading */
  .ep-loading-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    padding: 64px 0;
    color: #9ca3af;
    font-size: 0.9rem;
  }

  /* Card */
  .ep-card {
    background: #1a1f2e;
    border: 1px solid rgba(255,255,255,0.08);
    border-radius: 14px;
    padding: 32px;
    display: flex;
    flex-direction: column;
    gap: 28px;
  }

  /* Avatar */
  .ep-avatar-section {
    display: flex;
    align-items: center;
    gap: 20px;
  }

  .ep-avatar-btn {
    position: relative;
    width: 80px;
    height: 80px;
    border-radius: 50%;
    cursor: pointer;
    flex-shrink: 0;
    overflow: hidden;
    display: block;
  }

  .ep-avatar-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    border-radius: 50%;
  }

  .ep-avatar-placeholder {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: linear-gradient(135deg, #3b82f6, #8b5cf6);
    color: #fff;
    font-size: 1.6rem;
    font-weight: 700;
    letter-spacing: 0.05em;
    border-radius: 50%;
    user-select: none;
  }

  .ep-avatar-overlay {
    position: absolute;
    inset: 0;
    background: rgba(0,0,0,0.45);
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    opacity: 0;
    transition: opacity 0.18s;
  }
  .ep-avatar-btn:hover .ep-avatar-overlay { opacity: 1; }
  .ep-camera-icon { font-size: 1.4rem; }

  .ep-hidden-input {
    position: absolute;
    inset: 0;
    opacity: 0;
    cursor: pointer;
    width: 100%;
    height: 100%;
  }

  .ep-avatar-meta { display: flex; flex-direction: column; gap: 4px; }
  .ep-avatar-label { font-size: 0.95rem; font-weight: 600; color: #e5e7eb; margin: 0; }
  .ep-avatar-hint  { font-size: 0.775rem; color: #6b7280; margin: 0; }

  /* Fields */
  .ep-fields { display: flex; flex-direction: column; gap: 18px; }

  .ep-field-group { display: flex; flex-direction: column; gap: 6px; }

  .ep-label {
    font-size: 0.8rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: #9ca3af;
  }

  .ep-select,
  .ep-input {
    background: #111827;
    border: 1px solid rgba(255,255,255,0.1);
    border-radius: 8px;
    padding: 10px 14px;
    font-size: 0.92rem;
    color: #e5e7eb;
    width: 100%;
    box-sizing: border-box;
    transition: border-color 0.15s, box-shadow 0.15s;
    appearance: auto;
  }
  .ep-select:focus,
  .ep-input:focus {
    outline: none;
    border-color: #3b82f6;
    box-shadow: 0 0 0 3px rgba(59,130,246,0.2);
  }
  .ep-input::placeholder { color: #4b5563; }

  /* Actions */
  .ep-actions {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }

  .ep-save-btn {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 10px 24px;
    background: linear-gradient(135deg, #3b82f6, #2563eb);
    color: #fff;
    font-size: 0.9rem;
    font-weight: 600;
    border: none;
    border-radius: 8px;
    cursor: pointer;
    transition: opacity 0.15s, transform 0.1s;
  }
  .ep-save-btn:disabled { opacity: 0.45; cursor: not-allowed; transform: none; }
  .ep-save-btn:not(:disabled):hover { opacity: 0.9; transform: translateY(-1px); }

  .ep-discard-btn {
    padding: 10px 18px;
    background: transparent;
    color: #9ca3af;
    font-size: 0.875rem;
    border: 1px solid rgba(255,255,255,0.1);
    border-radius: 8px;
    cursor: pointer;
    transition: color 0.15s, border-color 0.15s;
  }
  .ep-discard-btn:hover { color: #e5e7eb; border-color: rgba(255,255,255,0.25); }

  /* Spinners */
  .ep-spinner {
    display: inline-block;
    border-radius: 50%;
    border-style: solid;
    border-color: rgba(255,255,255,0.2);
    border-top-color: #fff;
    animation: ep-spin 0.7s linear infinite;
  }
  .ep-spinner--lg     { width: 36px; height: 36px; border-width: 4px; }
  .ep-spinner--avatar {
    width: 40px; height: 40px; border-width: 4px;
    position: absolute; top: 50%; left: 50%;
    transform: translate(-50%, -50%);
  }
  .ep-spinner--sm { width: 14px; height: 14px; border-width: 2px; }

  @keyframes ep-spin     { to { transform: rotate(360deg); } }
  @keyframes ep-slide-in { from { opacity: 0; transform: translateY(-6px); } to { opacity: 1; transform: none; } }
</style>
