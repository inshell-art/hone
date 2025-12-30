# Hone: A Minimalist Editor to Polish Ideas into Principles

Hone is a minimalist writing app for facets and articles. A facet is a titled idea (a line that starts with `$`). Articles collect facets and the narrative between them so you can revisit, refine, and hone your thinking.

## Try It Now

Visit the hosted app: https://hone.inshell.art

## Key Features

- **Facets and articles**: Facet titles are the only formatting; everything else is plain text.
- **Slash commands**: Type `/` at the start of a line to open the palette.
  - `/create` inserts a `$` facet title.
  - `/update` saves the current facet to the library.
  - `/hone` inserts another facet into the current one.
- **Honed from blocks**: Honed inserts are wrapped as:
  - `--- honed-from: <id> | <title> | <timestamp> ---`
  - `--- end honed-from ---`
- **Facet library**: The Facets tab lists updated time and a "Honed from" history with similarity scores.
- **Local-first**: Data lives in your browser LocalStorage. Import/Export is available in the footer.

## Quick Start (Local Dev)

```bash
git clone https://github.com/inshell-art/hone.git
cd hone
npm install
npm run dev
```

Open http://localhost:5173

## Optional: Firebase Hosting Emulator

```bash
npm run emu
```

This builds and serves through the Firebase Hosting emulator at http://localhost:5002

## Tests

```bash
npm run test          # Cypress E2E (spins up Vite on a free port)
npm run test:unit     # Vitest
npm run test:coverage # Cypress E2E + coverage report
```

## Usage Tips

- Start a facet by typing `$` at the start of a line or using `/create`.
- Update a facet by placing the caret inside it, then type `/` at line start and choose `/update`.
- Hone a facet the same way with `/hone`, then pick a facet from the list.

## Storage and Privacy

- Everything is stored locally in LocalStorage.
- Autosave runs within about a second.
- Import/Export lets you back up or move data.

## License

MIT. See `LICENSE`.

## Contributing

Issues and pull requests are welcome.

## Inspired By

- Vim (modes)
- Notion (blocks)
- Obsidian (links)
- Day One (journaling)
- iA Writer (focus)

## Contact

Use GitHub Issues: https://github.com/inshell-art/hone/issues
