// Cloudflare Worker for oilfieldplanner.factorygamefan.com.
//
// The site is a Worker with static assets (see ../wrangler.jsonc). A request
// that matches a file in dist/ is served straight from the asset store, and
// this script never runs. The script runs only when nothing matched. Its one
// job is the single-page fallback: client-side routes such as /oil-field have
// no file behind them, so it returns the SPA shell (dist/404.html, a copy of
// index.html made by `npm run build`) with status 404, and vue-router takes
// over in the browser. The 404 status is on purpose. It is what the site
// returned on Cloudflare Pages, so crawlers see no change from the move.
//
// This file used to be public/_worker.js, a Pages "advanced mode" Worker. It
// also 301-redirected the old factoriotools-5jg.pages.dev host to the custom
// domain. A Worker never receives requests for a pages.dev host, so that code
// could never run here and was removed. The old Pages project does that
// redirect now, with a one-line _redirects file. See issue #126.

export default {
  async fetch(request, env) {
    // Ask the asset store first. Without run_worker_first this finds nothing,
    // since a match would never have reached the script, but it keeps the
    // script correct if run_worker_first is ever turned on.
    const response = await env.ASSETS.fetch(request)
    if (response.status !== 404) {
      return response
    }

    // Ask for the shell as /404, not /404.html. The asset store answers
    // /404.html with a 307 redirect to /404 (its default html_handling), so
    // the extensionless path skips that extra hop.
    const shell = await env.ASSETS.fetch(new URL("/404", request.url))
    return new Response(shell.body, { status: 404, headers: shell.headers })
  },
}
