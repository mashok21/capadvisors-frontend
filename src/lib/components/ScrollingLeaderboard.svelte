<script lang="ts">
	import { onMount } from 'svelte';
	import { fade } from 'svelte/transition';

	// Define explicit types matching our backend relational schemas
	interface LeaderboardTickerEntry {
		student_id: string;
		display_name: string;
		rating: number;
		articleship_firm: string;
		avatar_url: string;
	}

	// Svelte 5 state runes
	let leaders = $state<LeaderboardTickerEntry[]>([]);
	let isLoading = $state(true);
	let errorOccurred = $state(false);

	// Derived rune: Duplicate the array to create a seamless infinite loop illusion in CSS
	let marqueeList = $derived([...leaders, ...leaders]);

	onMount(async () => {
		try {
			const baseApiUrl = import.meta.env.VITE_API_URL || 'http://localhost:3000';
			const res = await fetch(`${baseApiUrl}/api/leaderboard`);
			if (!res.ok) throw new Error('Failed to stream ranking metrics');
			const data = await res.json();
			
			// Take top 10 for the high-impact teaser banner
			leaders = data.slice(0, 10);
		} catch (err) {
			console.error('Ticker sync error:', err);
			errorOccurred = true;
		} finally {
			isLoading = false;
		}
	});

	// Helper to resolve firm accents matching the core leaderboard design matrix
	function getFirmColor(firm: string): string {
		switch (firm) {
			case 'Deloitte': return 'border-emerald-500 text-emerald-400';
			case 'KPMG': return 'border-blue-500 text-blue-400';
			case 'EY': return 'border-yellow-500 text-yellow-400';
			case 'PwC': return 'border-orange-500 text-orange-400';
			default: return 'border-slate-600 text-slate-400';
		}
	}
</script>

<div class="ticker-wrapper bg-slate-900 border-y border-slate-800 py-3 overflow-hidden relative flex items-center">
	<div class="absolute left-0 top-0 bottom-0 w-16 bg-gradient-to-r from-slate-900 to-transparent z-10 pointer-events-none"></div>

	{#if isLoading}
		<div class="flex gap-8 px-4 animate-pulse w-full justify-center" out:fade>
			{#each Array(5) as _}
				<div class="h-6 w-36 bg-slate-800 rounded-full"></div>
			{/each}
		</div>
	{:else if !errorOccurred && marqueeList.length > 0}
		<div class="marquee-track flex gap-6 whitespace-nowrap items-center hover:pause-animation">
			{#each marqueeList as leader, i}
				<div class="ticker-item flex items-center gap-3 bg-slate-950/60 border border-slate-800/80 rounded-full px-4 py-1.5 shadow-md">
					<span class="text-xs font-bold text-slate-500">
						#{(i % leaders.length) + 1}
					</span>

					<div class="w-7 h-7 rounded-full border-2 overflow-hidden flex items-center justify-center bg-slate-800 text-[10px] font-bold ${getFirmColor(leader.articleship_firm)}">
						{#if leader.avatar_url}
							<img src={leader.avatar_url} alt={leader.display_name} class="w-full h-full object-cover" />
						{:else}
							{leader.display_name.slice(0, 2).toUpperCase()}
						{/if}
					</div>

					<div class="flex flex-col text-left">
						<span class="text-xs font-semibold text-slate-200">{leader.display_name}</span>
						<span class="text-[10px] text-slate-400 font-mono tracking-wider">{Math.round(leader.rating)} pts</span>
					</div>

					{#if leader.articleship_firm !== 'None / Other'}
						<span class="text-[9px] px-2 py-0.5 rounded-full font-medium bg-slate-900 border border-slate-800 uppercase tracking-tight ${getFirmColor(leader.articleship_firm)}">
							{leader.articleship_firm}
						</span>
					{/if}
				</div>
			{/each}
		</div>
	{/if}

	<div class="absolute right-0 top-0 bottom-0 w-16 bg-gradient-to-l from-slate-900 to-transparent z-10 pointer-events-none"></div>
</div>

<style>
	.ticker-wrapper {
		width: 100%;
		mask-image: linear-gradient(to right, transparent, black 10%, black 90%, transparent);
	}

	.marquee-track {
		display: flex;
		width: max-content;
		animation: scroll 30s linear infinite;
	}

	.hover\:pause-animation:hover {
		animation-play-state: paused;
	}

	/* Ultra-smooth hardware accelerated scrolling loop translations */
	@keyframes scroll {
		0% {
			transform: translateX(0);
		}
		100% {
			transform: translateX(-50%);
		}
	}
</style>