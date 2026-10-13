// Screenshot switches for the browser demo (`?demo&panel=settings&banner=paused`); null outside it.
// Kept apart from ./index.ts so the app can read them without loading the mocks.
export function demoParam(name: string): string | null {
  const q = new URLSearchParams(location.search);
  return q.has("demo") ? q.get(name) : null;
}
