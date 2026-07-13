// Cloudflare Pages "advanced mode" Worker.
//
// Its only job is to 301-redirect the retired *.pages.dev host to the custom
// domain. Everything else is served exactly as before via the Pages static
// asset pipeline (env.ASSETS), including the index.html -> 404.html single-page
// fallback that makes client-side routes like /oil-field work on hard loads.
//
// This file lives in public/ so Vite copies it to the build output root
// (dist/_worker.js), where Pages picks it up. Pages treats a root _worker.js as
// the Worker, not as a served static asset.

const OLD_HOST = "factoriotools-5jg.pages.dev";
const NEW_HOST = "oilfieldplanner.factorygamefan.com";

export default {
  async fetch(request, env) {
    const url = new URL(request.url);

    // Redirect the old production host to the new domain, preserving the full
    // path, query string and hash. Preview/branch deployments live on
    // <hash>.factoriotools-5jg.pages.dev and are intentionally left alone.
    if (url.hostname === OLD_HOST) {
      url.hostname = NEW_HOST;
      return Response.redirect(url.toString(), 301);
    }

    // Serve the matching static asset. For paths with no asset (client-side
    // routes such as /oil-field), fall back to the SPA shell so vue-router can
    // take over - mirroring the pre-Worker behavior where Pages served 404.html.
    const response = await env.ASSETS.fetch(request);
    if (response.status === 404) {
      const shell = await env.ASSETS.fetch(new URL("/404.html", url.origin));
      return new Response(shell.body, { status: 404, headers: shell.headers });
    }
    return response;
  },
};
