# Docklet website

Static launch and download site for Docklet. The visual direction comes from the
Gallery variant in `portfolio-ui-prototype.html`.

## Preview

There is no build step. Serve the folder with any static file server, for example:

```powershell
bunx serve website
```

Then open the printed local URL. Preview it through a server rather than
double-clicking `index.html`; the self-hosted font files are blocked by browser
CORS rules on `file://` pages.

## Release links

The download buttons point at the `v0.1.0` release assets on GitHub. Update the
versioned URLs in `index.html` when a new release is published.
