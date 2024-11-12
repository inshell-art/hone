# Hone: A Minimalist Editor to Polish Ideas into Principles

Hone is a unique editor designed to help you refine your ideas until they transform into core principles. By providing a simple structure focused on facets and articles, Hone facilitates deep reflection, honing thoughts into refined insights.

## Try It Now!

You can try Hone right away without installing anything. Simply visit our hosted site:
[hone.inshell.art](https://hone.inshell.art)

## Key Features

- **Facets and Articles**: Hone offers two text formats—facets and articles. A facet is a highlighted block of text that starts with `$`, representing a specific idea. Articles are composed of one or more facets, allowing you to tell stories, explore ideas, and capture your thoughts in detail.
- **Hone Your Thoughts**: Each facet can be "honed" by inserting another facet into it. This creates a chain of reflection, allowing ideas to be polished repeatedly, deepening understanding and leading to core principles.
- **Simplicity by Design**: With no added formatting beyond the facet titles, Hone maintains a clear and focused approach to writing, adhering to Occam's razor: simplicity to the extreme. There are no features like bold, italics, or underline—just you, your thoughts, and the facets that capture them.
- **Local and Private Storage**: Hone saves all your work locally, directly to your browser's LocalStorage. There is no cloud storage or account registration—all your data remains private and secure on your device.

## Installation and Setup

To use Hone, follow these steps:

1. **Clone the Repository**:

   ```
   git clone https://github.com/yourusername/hone.git
   cd hone
   ```

2. **Install Dependencies**:

   ```
   npm install
   ```

3. **Run the Application**:

   ```
   npm start
   ```

   This command will start a local server. Open your browser and navigate to `http://localhost:4173` to start using Hone.

## Using Hone

### Two Formats: Facets and Articles

- **Facet**: Begin a facet by starting a line with `$`. This denotes a distinct idea or section of an article.
- **Article**: Articles consist of multiple facets, with a title that precedes the facets. The section between the article title and the first facet serves as an introduction or setup.

### How to Hone a Facet

- To hone a facet, place your cursor at the beginning of any facet line and press `Cmd + Enter` (on Mac) or `Ctrl + Enter` (on Windows). A panel will appear, showing all available facets.
- Each facet is accompanied by a **similarity percentage** calculated using **Jaccard Similarity**, indicating how closely related it is to the current facet.

### Recording Honed Facets

- When a facet is honed, Hone records this action. In the **Facets tab**, the hierarchy of facets is displayed, showing which facets have been honed and how many times.
- Facets are sorted by the number of times they have been honed, emphasizing their level of development.

## Storage and Privacy

- All data in Hone is stored in your browser's **LocalStorage**. There are no external servers or cloud services involved, so your data remains private and secure.
- **Automatic Saving**: Hone automatically saves any changes within one second, ensuring that nothing is lost.

## Getting Started

- Click **"Create Article"** at the top-right corner of the home page to start writing.
- Begin your article with facets by typing `$` at the start of a line, hone your ideas, and watch them evolve.

## License

This project is licensed under the MIT License. See the `LICENSE` file for more details.

## Contributing

We welcome contributions! Feel free to open an issue or submit a pull request to improve Hone.

## Inspired by:

- [Vim](https://www.vim.org/): The modes switching.
- [Notion](https://www.notion.so/): The block.
- [Obsidian](https://obsidian.md/): The double bracket link.
- [Day One](https://dayoneapp.com/): The journal daily to create scenarios to polish ideas.
- [iA Writer](https://ia.net/writer): The focus mode.

## Contact

For more information, suggestions, or contributions, please reach out via [GitHub Issues](https://github.com/inshell-art/hone/issues).
