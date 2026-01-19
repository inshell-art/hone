# Hone Summary

Hone is a minimalist, local-first writing app built around two structures: facets and articles. A facet is a titled idea that starts with a `$` line; everything until the next facet title belongs to that facet. Articles collect multiple facets (plus any intro text) so ideas can be written, refined, and revisited as a whole.

## Facets in Hone

- **Facet title**: A single line starting with `$` marks the facet title and defines the facet boundary.
- **Slash commands**: Type `/` at the start of a line to open the palette.
  - `/create` inserts a facet title (`$`).
  - `/update` saves the current facet to the library.
  - `/hone` inserts another facet into the current one.
- **Honed-from blocks**: Honed inserts are wrapped with a header/footer:
  - `--- honed-from: <id> | <title> | <timestamp> ---`
  - `--- end honed-from ---`
- **Facet library**: The Facets tab lists all saved facets with updated times and a honed-from history.

## Storage and Privacy

All data stays in the browser LocalStorage. There are no accounts or servers, and Import/Export in the footer can be used for backups or transfers.
