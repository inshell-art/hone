# Facet Library v2 (Local-Only)

- **LocalStorage keys**
  - `honeData` – autosaved articles (Lexical editor state).
  - `honeFacetsLibraryV2` – the facet library (source of truth for the Facets tab).
  - `facetsData` – read-only payload when running in facets-only mode (`VITE_IS_FACETS === "true"`).

- **Data model (library)**
  - `FacetsLibraryState { version: 2; updatedAt; facetsById }`
  - `FacetLibraryItem { facetId, title, bodyText, updatedAt, honedFrom: HoneEdge[] }`
  - `HoneEdge { fromFacetId, honedAt }`
  - Identity is the `facetId`; `/update` upserts by `facetId`; `/hone` records edges by `facetId`.

- **Facet lifecycle**
  - Draft facets live in articles and autosave frequently.
  - `/update` promotes the current facet (title + body) into the library and refreshes its `updatedAt`, preserving `honedFrom`.
  - `/hone` links the current facet to another library facet (`honedFrom` edge) and inserts the source facet text into the editor; if the target facet is not yet in the library, `/update` first.

- **Facets tab (Facet Library view)**
  - Reads from `honeFacetsLibraryV2` only.
  - Top-level entries sorted by `updatedAt` (desc).
  - Each entry shows title and updated time, plus a one-hop “Honed from” list:
    - Items sorted by `honedAt` (desc).
    - Each item shows title and similarity % vs the parent facet.
  - No nested honed-from-of-honed-from rendering.

- **Slash commands (keyboard-first)**
  - Trigger: type `/` at the start of a line to open the palette; Up/Down to select, Enter to run, Esc to close, typing filters.
  - `/facet`: insert a new facet title node (`$`-style) and focus it.
  - `/update`: upsert the current facet into the library (message shown if not inside a facet).
  - `/hone`: pick a library facet by similarity to hone the current facet; adds a hone edge and inserts the source text with a delimiter block. Shows a message if library is empty or cursor is outside a facet.
