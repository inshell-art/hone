# Publishing in Hone

Hone keeps two versions of an article:

- **Draft**: the mutable editor state saved in LocalStorage (`honeData`).
- **Edition**: an immutable snapshot created on publish (`honeArticleEditionsV1`).

## Publish an edition

1. Open an article draft.
2. Type `/` at the start of a line.
3. Choose `/publish`.

Each publish creates a new version (`v1`, `v2`, ...). Previous editions are never modified.

## Stable URLs

Published editions have stable, shareable URLs:

- `/a/:articleId/v/:version`

In read-only mode, the app renders these editions without editing.

## Notes

- Publishing does not update facets. Use `/update` explicitly when you want a facet saved to the library.
- Export includes drafts, facet library, and editions so a hosted read-only site can render editions.
