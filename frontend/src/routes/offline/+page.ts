// The service worker serves this shell for navigations it cannot reach the server for;
// the client router then renders the requested page from saved catalogs.
export const prerender = true;
export const ssr = false;
