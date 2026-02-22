# Agent Tools

This document outlines the tools being developed for agent use, their implementation steps, and specific test scenarios to validate their effectiveness.

## General Agent Interaction Scenarios

Before delving into specific tools, these general test scenarios should be verified for all tools to ensure they are robust for agentic use:

- **Input Validation:** Test how tools handle missing, malformed, or unexpected input parameters from agents.
- **Error Reporting:** Ensure tools return clear, actionable error messages in a structured format (e.g., JSON) that an agent can parse and use to self-correct.
- **Output Consistency:** Verify that output formats remain consistent across different successful calls to simplify agent parsing logic.

---

## 1. Markdown Search Tool

This tool will allow agents to search through markdown files for specific content, such as headers, links, or code blocks. It should support queries like "Find all headers in this file" or "Extract all links from this document". The output should be structured (e.g., JSON) to facilitate further processing by agents.

### Implementation Checklist
- [ ] Define the tool's interface and expected input/output formats.
- [ ] Implement the core functionality to parse markdown files and extract relevant information based on queries.
- [ ] Add support for different query types (headers, links, code blocks, etc.).
- [ ] Write tests to ensure the tool works correctly with various markdown structures.
- [ ] Document the tool's usage and capabilities in `tooli_feedback.md` for agent consumption.

### Test Scenarios for Agents
- **Header Extraction:** Agent requests all headers from a complex, multi-level markdown document. Verify the nesting level and text are captured correctly.
- **Link Discovery:** Agent asks to extract all external links from a `README.md`. Test with various link formats (inline, reference-style, autolinks).
- **Code Block Filtering:** Agent searches for all `python` code blocks to extract implementation details. Verify only the requested language blocks are returned.

---

## 2. Image Manipulation Tool

This tool will provide basic image manipulation capabilities, such as resizing, cropping, and format conversion. Agents could use this tool to prepare images for specific use cases, like generating thumbnails or converting images to a web-friendly format.

### Implementation Checklist
- [ ] Image resizing with specified dimensions or aspect ratio.
- [ ] Cropping images based on given coordinates or predefined regions (e.g., center crop).
- [ ] Format conversion between common image formats (JPEG, PNG, GIF).
- [ ] Support for batch processing of multiple images at once.
- [ ] Error handling for unsupported formats or invalid input parameters.
- [ ] Comprehensive tests to validate functionality across different image types and edge cases.
- [ ] Documentation in `tooli_feedback.md` to guide agents on how to use the tool effectively.

### Test Scenarios for Agents
- **Thumbnail Generation:** Agent needs to resize a large high-resolution image to a 200x200 PNG thumbnail. Verify aspect ratio handling (e.g., padding vs. cropping).
- **Batch Conversion:** Agent processes a directory of `.bmp` files, converting them all to `.webp` for web optimization.
- **Boundary Testing:** Agent attempts to crop an image using coordinates that exceed the original dimensions. Verify the tool returns a graceful error or clamps the coordinates.

---

## 3. Markdown Cleaner Tool

This tool will clean up markdown files by unwrapping paragraphs, removing unnecessary span/div tags, and converting EPUB index links to a more agent-friendly format. This will help agents process markdown content more effectively by providing cleaner input.

### Implementation Checklist
- [ ] Implement paragraph unwrapping to remove unnecessary line breaks while preserving content structure.
- [ ] Create functionality to identify and remove redundant span/div tags that do not contribute to the document's meaning.
- [ ] Develop a method to convert EPUB index links into a format that is easier for agents to parse and utilize.
- [ ] Ensure the tool can handle various markdown structures and edge cases without breaking the content.
- [ ] Write tests to validate the cleaning process across different markdown files and scenarios.
- [ ] Document the tool's capabilities and usage instructions in `tooli_feedback.md` for agent reference.

### Test Scenarios for Agents
- **EPUB Cleanup:** Agent processes a markdown file exported from an EPUB that contains excessive `<span>` tags and hard-wrapped lines. Verify the output is "clean" and readable.
- **Paragraph Unwrapping:** Agent identifies a file with 80-character hard-wraps and requests unwrapping. Verify that lists and blockquotes remain intact.
- **Link Normalization:** Agent converts complex EPUB-style index links into standard markdown links for easier navigation.
