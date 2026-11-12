# Private capture collector

The Worker accepts opt-in browser captures and writes one JSON file per capture to `incoming/` in the private Website Analysis Engine repository. It never receives full HTML or runs a model. Contributors do not need a GitHub or Cloudflare account.

## Owner setup

1. Create a GitHub App on the repository owner account. Grant only **Contents: read and write**, install it only on `Detractless/Website-Analysis-Engine`, and copy its app ID, installation ID, and private key.
2. In Chrome, load `src/browser-extension/` unpacked and copy its extension ID. Set `EXTENSION_ORIGIN` to `chrome-extension://<that-id>`, replace the app and installation IDs, and choose a rate-limit namespace ID unique to your Cloudflare account.
3. Create a Cloudflare account for the project owner if needed. Set the private key as a Worker secret with `npx wrangler secret put GITHUB_APP_PRIVATE_KEY`. Do not put the private key in this repo or the extension.
4. Deploy with `npx wrangler deploy` and copy the Worker URL. Replace `COLLECTOR_ENDPOINT` in `src/browser-extension/collector-config.js` with that URL plus `/v1/captures`, then reload the extension and refresh open pages.

The Worker rate limits by request IP because contributors have no account. Cloudflare's rate limits are approximate and local to each Cloudflare location; people sharing an IP may share a limit. See [Cloudflare's rate-limiting notes](https://developers.cloudflare.com/workers/runtime-apis/bindings/rate-limit/).

## Data and removal

The Worker strips URL fragments, common tracking parameters, and query keys that look like credentials before committing. It writes captures to the private repo's `incoming/<capture-id>.json` path and stores a hash of a random deletion receipt. The extension can request removal with that receipt. GitHub's new commit removes the file from the current branch, but older commits can retain its contents; fully erasing history needs a separate repository-history cleanup.
