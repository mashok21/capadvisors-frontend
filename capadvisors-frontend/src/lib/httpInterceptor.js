import { auth } from './auth.svelte.js';

// Requests to these endpoints return 401 for *invalid credentials*, not an
// expired session — they must not trigger a forced logout/redirect.
const AUTH_ENDPOINTS = ['/api/auth/login', '/api/auth/register'];

function isAuthEndpoint(input) {
  const url = typeof input === 'string' ? input : input?.url ?? '';
  return AUTH_ENDPOINTS.some((path) => url.includes(path));
}

// Guard against re-patching window.fetch across Vite HMR reloads.
if (!window.__capadvisorsFetchPatched) {
  window.__capadvisorsFetchPatched = true;
  const nativeFetch = window.fetch.bind(window);

  window.fetch = async (input, init) => {
    const response = await nativeFetch(input, init);

    if (response.status === 401 && !isAuthEndpoint(input)) {
      auth.logout();
      window.location.href = '/login';
    }

    return response;
  };
}
