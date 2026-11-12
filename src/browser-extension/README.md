# Leko Enforcer page collection

This is a first Chrome Manifest V3 collector package. It starts off, captures readable page text only after opt-in, and sends it to the project collector. It does not summarize pages or save full HTML. Site exclusions, a local retry queue, local queue removal, and deletion receipts are included.

## Local setup

1. Deploy the collector described in `../collector-worker/README.md`.
2. Set `COLLECTOR_ENDPOINT` in `collector-config.js` to the Worker URL. This URL is public configuration; it is not a credential.
3. In Chrome, open `chrome://extensions`, enable Developer mode, and load this folder with **Load unpacked**. Refresh open pages after installing so Chrome adds the content script.
4. In the extension popup, read the disclosure and click **Turn collection on** only if you want to contribute.

The extension asks for access to HTTP and HTTPS pages so it can collect after opt-in. The Worker accepts only the configured extension origin, but CORS is not authentication; request limits and body validation also apply.

Each submitted capture creates one commit and file in the private GitHub inbox. Deleting a sent capture removes it from the current branch; older Git commits can still retain it until the repository history is cleaned. The inbox has no automatic expiration yet, so the owner must review and clean it regularly.
