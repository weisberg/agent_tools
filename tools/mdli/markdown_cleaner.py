#!/usr/bin/env -S uv run
# /// script
# dependencies = []
# ///

import argparse
import os
import re
import sys


def is_special_block(block: str) -> bool:
    """Check if a block should be preserved without unwrapping."""
    lines = block.split('\n')
    first_line = lines[0].strip()

    # Heading
    if first_line.startswith('#'):
        return True

    # Fenced code block
    if first_line.startswith('```') or first_line.startswith('~~~'):
        return True

    # Indented code block (all lines start with 4 spaces or tab)
    if all(line.startswith('    ') or line.startswith('\t') or line == '' for line in lines):
        return True

    # List (unordered or ordered)
    if re.match(r'^[\-\*\+]\s', first_line) or re.match(r'^\d+\.\s', first_line):
        return True

    # Blockquote
    if first_line.startswith('>'):
        return True

    # Table (contains |)
    if '|' in first_line:
        return True

    # HTML block
    if first_line.startswith('<'):
        return True

    return False


def unwrap_paragraph(block: str) -> str:
    """Unwrap a paragraph by joining lines with spaces."""
    # Replace single newlines with spaces
    unwrapped = block.replace('\n', ' ')
    # Collapse multiple spaces into one
    unwrapped = re.sub(r' +', ' ', unwrapped)
    return unwrapped.strip()


def remove_unreferenced_spans(content: str, verbose: bool = False) -> str:
    """Remove <span> tags unless their id is referenced by a link in the document."""
    # Find all internal anchor references: [text](#id) or href="#id"
    markdown_links = re.findall(r'\[.*?\]\(#([^)]+)\)', content)
    html_links = re.findall(r'href="#([^"]+)"', content)
    referenced_ids = set(markdown_links + html_links)

    if verbose:
        print(f"[spans] Found {len(referenced_ids)} referenced anchor IDs", file=sys.stderr)

    removed_count = 0
    kept_count = 0

    def replace_span(match):
        nonlocal removed_count, kept_count
        full_match = match.group(0)
        # Extract id attribute if present
        id_match = re.search(r'id=["\']([^"\']+)["\']', full_match)
        if id_match:
            span_id = id_match.group(1)
            if span_id in referenced_ids:
                # Keep this span, it's referenced
                kept_count += 1
                if verbose:
                    print(f"[spans] KEPT: {span_id} (referenced)", file=sys.stderr)
                return full_match
        # Remove span tags but keep content
        removed_count += 1
        if verbose:
            span_id = id_match.group(1) if id_match else "(no id)"
            print(f"[spans] REMOVED: {span_id}", file=sys.stderr)
        inner_content = match.group(1)
        return inner_content

    # Match <span ...>content</span> including nested content
    # Use non-greedy matching and handle self-closing or empty spans too
    result = re.sub(r'<span[^>]*>(.*?)</span>', replace_span, content, flags=re.DOTALL)

    # Also remove self-closing spans <span ... /> that aren't referenced
    def replace_self_closing_span(match):
        nonlocal removed_count, kept_count
        full_match = match.group(0)
        id_match = re.search(r'id=["\']([^"\']+)["\']', full_match)
        if id_match and id_match.group(1) in referenced_ids:
            kept_count += 1
            if verbose:
                print(f"[spans] KEPT (self-closing): {id_match.group(1)}", file=sys.stderr)
            return full_match
        removed_count += 1
        if verbose:
            span_id = id_match.group(1) if id_match else "(no id)"
            print(f"[spans] REMOVED (self-closing): {span_id}", file=sys.stderr)
        return ''

    result = re.sub(r'<span[^>]*/>', replace_self_closing_span, result)

    if verbose:
        print(f"[spans] Summary: {removed_count} removed, {kept_count} kept", file=sys.stderr)

    return result


def remove_divs(content: str, verbose: bool = False) -> str:
    """Remove all <div> tags, keeping their content."""
    opening_count = len(re.findall(r'<div[^>]*>', content))
    closing_count = len(re.findall(r'</div>', content))

    # Remove opening div tags
    result = re.sub(r'<div[^>]*>', '', content)
    # Remove closing div tags
    result = re.sub(r'</div>', '', result)

    if verbose:
        print(f"[divs] Removed {opening_count} opening and {closing_count} closing div tags", file=sys.stderr)

    return result


def heading_to_gfm_anchor(heading_text: str) -> str:
    """Convert heading text to GitHub-flavored Markdown anchor ID."""
    # Remove leading # and whitespace
    text = re.sub(r'^#+\s*', '', heading_text)
    # Convert to lowercase
    text = text.lower()
    # Remove punctuation except hyphens and spaces
    text = re.sub(r'[^\w\s-]', '', text)
    # Replace spaces with hyphens
    text = re.sub(r'\s+', '-', text)
    # Remove leading/trailing hyphens
    text = text.strip('-')
    return text


def convert_index_links(content: str, verbose: bool = False) -> str:
    """Convert EPUB index links to GFM heading links.

    Finds links like: term [pagenum](#anchor)
    Converts to: [term](#heading-anchor)

    Also converts any links pointing to span IDs inside headings.
    """
    print("Converting index links to heading anchors...", file=sys.stderr)

    # Build a map of all headings in order, tracking duplicates for GFM anchors
    # Also extract span IDs that are inside each heading
    heading_pattern = re.compile(r'^(#{1,6})\s+(.+)$', re.MULTILINE)
    headings = []  # List of (position, heading_text, gfm_anchor)
    heading_span_ids = {}  # Map span IDs inside headings to their GFM anchor
    anchor_counts = {}  # Track duplicate anchors

    for match in heading_pattern.finditer(content):
        heading_text = match.group(2).strip()

        # Extract span IDs from this heading
        span_ids_in_heading = re.findall(r'<span[^>]*id=["\']([^"\']+)["\'][^>]*>', heading_text)

        # Remove span tags from heading text before generating anchor
        heading_text_clean = re.sub(r'<span[^>]*>|</span>', '', heading_text)
        base_anchor = heading_to_gfm_anchor(heading_text_clean)

        # Handle duplicate headings
        if base_anchor in anchor_counts:
            anchor_counts[base_anchor] += 1
            gfm_anchor = f"{base_anchor}-{anchor_counts[base_anchor]}"
        else:
            anchor_counts[base_anchor] = 0
            gfm_anchor = base_anchor

        headings.append((match.start(), heading_text, gfm_anchor))

        # Map all span IDs in this heading to the GFM anchor
        for span_id in span_ids_in_heading:
            heading_span_ids[span_id] = gfm_anchor

    if verbose:
        print(f"[index] Found {len(headings)} headings", file=sys.stderr)
        print(f"[index] Found {len(heading_span_ids)} span IDs inside headings", file=sys.stderr)

    # Build a map of span IDs to their positions (for non-heading spans)
    span_pattern = re.compile(r'<span[^>]*id=["\']([^"\']+)["\'][^>]*>')
    span_positions = {}  # span_id -> position

    for match in span_pattern.finditer(content):
        span_id = match.group(1)
        span_positions[span_id] = match.start()

    if verbose:
        print(f"[index] Found {len(span_positions)} total spans with IDs", file=sys.stderr)

    # Find the heading that precedes each span
    def find_preceding_heading(span_pos):
        preceding = None
        for pos, heading_text, gfm_anchor in headings:
            if pos < span_pos:
                preceding = (heading_text, gfm_anchor)
            else:
                break
        return preceding

    # Build map of span IDs to GFM anchors (for spans in body text)
    span_to_heading = {}
    for span_id, span_pos in span_positions.items():
        # Skip spans that are inside headings (already mapped)
        if span_id in heading_span_ids:
            continue
        heading_info = find_preceding_heading(span_pos)
        if heading_info:
            span_to_heading[span_id] = heading_info[1]  # gfm_anchor

    # Merge heading span IDs into the map
    span_to_heading.update(heading_span_ids)

    # First, convert any direct links to heading span IDs: [text](#heading-span-id) -> [text](#gfm-anchor)
    heading_link_converted = 0

    def replace_heading_span_link(match):
        nonlocal heading_link_converted
        link_text = match.group(1)
        anchor_id = match.group(2)

        if anchor_id in heading_span_ids:
            gfm_anchor = heading_span_ids[anchor_id]
            heading_link_converted += 1
            if verbose:
                print(f"[index] HEADING LINK: '[{link_text}](#{anchor_id})' -> '[{link_text}](#{gfm_anchor})'", file=sys.stderr)
            return f'[{link_text}](#{gfm_anchor})'
        return match.group(0)

    # Match markdown links: [text](#anchor)
    link_pattern = re.compile(r'\[([^\]]+)\]\(#([^)]+)\)')
    result = link_pattern.sub(replace_heading_span_link, content)

    # Convert index links: "term [pagenum](#anchor)" -> "[term](#heading-anchor)"
    # Use [ ] instead of \s to avoid matching across newlines
    index_link_pattern = re.compile(r'(\w[\w ]*?)[ ]+\[\d+\]\(#([^)]+)\)')

    converted_count = 0
    unresolved_count = 0

    def replace_index_link(match):
        nonlocal converted_count, unresolved_count
        term = match.group(1).strip()
        anchor_id = match.group(2)

        if anchor_id in span_to_heading:
            gfm_anchor = span_to_heading[anchor_id]
            converted_count += 1
            if verbose:
                print(f"[index] CONVERTED: '{term}' -> #{gfm_anchor}", file=sys.stderr)
            return f'[{term}](#{gfm_anchor})'
        else:
            # Can't resolve, keep original
            unresolved_count += 1
            if verbose:
                print(f"[index] UNRESOLVED: '{term}' (anchor {anchor_id} not found)", file=sys.stderr)
            return match.group(0)

    result = index_link_pattern.sub(replace_index_link, result)

    print(f"Index links: {converted_count} converted, {unresolved_count} unresolved", file=sys.stderr)
    if heading_link_converted > 0:
        print(f"Heading span links: {heading_link_converted} converted", file=sys.stderr)

    # Clean up remaining index artifacts:
    # Remove IndexMarker links: [pagenum](#...idIndexMarker...)
    index_marker_pattern = re.compile(r'\[\d+\]\(#[^)]*idIndexMarker[^)]*\)')
    index_marker_count = len(index_marker_pattern.findall(result))
    result = index_marker_pattern.sub('', result)

    # Remove bold separators: **,** and **-**
    result = re.sub(r'\*\*,\*\*', '', result)
    result = re.sub(r'\*\*-\*\*', '', result)

    # Clean up any leftover whitespace from removals
    result = re.sub(r' +', ' ', result)  # Multiple spaces to single
    result = re.sub(r' +\n', '\n', result)  # Trailing spaces before newline

    if index_marker_count > 0:
        print(f"Index markers removed: {index_marker_count}", file=sys.stderr)

    return result


def clean_markdown(content: str, keep_divs: bool = False, convert_index: bool = False, verbose: bool = False) -> str:
    """Clean markdown by unwrapping paragraphs while preserving special blocks."""
    # Normalize line endings
    content = content.replace('\r\n', '\n').replace('\r', '\n')

    # Convert index links before removing spans (so converted links don't reference old anchors)
    if convert_index:
        if verbose:
            print("[clean] Converting index links...", file=sys.stderr)
        content = convert_index_links(content, verbose=verbose)

    # Remove unreferenced span tags from EPUB conversion
    if verbose:
        print("[clean] Removing unreferenced spans...", file=sys.stderr)
    content = remove_unreferenced_spans(content, verbose=verbose)

    # Remove div tags unless --keep-divs is specified
    if not keep_divs:
        if verbose:
            print("[clean] Removing div tags...", file=sys.stderr)
        content = remove_divs(content, verbose=verbose)
    elif verbose:
        print("[clean] Keeping div tags (--keep-divs)", file=sys.stderr)

    # Split on double newlines (paragraph separator)
    blocks = re.split(r'\n\n+', content)

    if verbose:
        print(f"[clean] Processing {len(blocks)} blocks...", file=sys.stderr)

    cleaned_blocks = []
    unwrapped_count = 0
    preserved_count = 0

    for block in blocks:
        if not block.strip():
            continue

        if is_special_block(block):
            cleaned_blocks.append(block)
            preserved_count += 1
        else:
            cleaned_blocks.append(unwrap_paragraph(block))
            unwrapped_count += 1

    if verbose:
        print(f"[clean] Paragraphs: {unwrapped_count} unwrapped, {preserved_count} preserved (special blocks)", file=sys.stderr)

    # Rejoin with double newlines
    result = '\n\n'.join(cleaned_blocks)

    # Ensure single trailing newline
    return result + '\n'


def main():
    parser = argparse.ArgumentParser(
        description='Clean markdown files by unwrapping hard-wrapped paragraphs.'
    )
    parser.add_argument('input', help='Input markdown file')
    parser.add_argument('-o', '--output', help='Output file (default: stdout)')
    parser.add_argument('-i', '--in-place', action='store_true',
                        help='Edit file in place')
    parser.add_argument('--keep-divs', action='store_true',
                        help='Keep <div> tags (by default they are removed)')
    parser.add_argument('--convert-index', action='store_true',
                        help='Convert EPUB index links to GFM heading links')
    parser.add_argument('-v', '--verbose', action='store_true',
                        help='Print verbose progress information to stderr')
    parser.add_argument('--overwrite', action='store_true',
                        help='Overwrite original file (original renamed with _original suffix)')

    args = parser.parse_args()

    # Read input file
    if args.verbose:
        print(f"[main] Reading: {args.input}", file=sys.stderr)
    with open(args.input, 'r', encoding='utf-8') as f:
        content = f.read()
    if args.verbose:
        print(f"[main] Input size: {len(content)} characters", file=sys.stderr)

    # Clean the markdown
    cleaned = clean_markdown(content, keep_divs=args.keep_divs, convert_index=args.convert_index, verbose=args.verbose)

    # Output
    if args.verbose:
        print(f"[main] Output size: {len(cleaned)} characters", file=sys.stderr)

    if args.in_place:
        if args.verbose:
            print(f"[main] Writing in-place: {args.input}", file=sys.stderr)
        with open(args.input, 'w', encoding='utf-8') as f:
            f.write(cleaned)
    elif args.overwrite:
        # Rename original file with _original suffix, write cleaned to original name
        base, ext = os.path.splitext(args.input)
        original_backup = f"{base}_original{ext}"
        if args.verbose:
            print(f"[main] Renaming original to: {original_backup}", file=sys.stderr)
        os.rename(args.input, original_backup)
        if args.verbose:
            print(f"[main] Writing cleaned to: {args.input}", file=sys.stderr)
        with open(args.input, 'w', encoding='utf-8') as f:
            f.write(cleaned)
    elif args.output:
        if args.verbose:
            print(f"[main] Writing to: {args.output}", file=sys.stderr)
        with open(args.output, 'w', encoding='utf-8') as f:
            f.write(cleaned)
    else:
        # Generate default output filename with "_cleaned" suffix
        base, ext = os.path.splitext(args.input)
        output_path = f"{base}_cleaned{ext}"
        if args.verbose:
            print(f"[main] Writing to: {output_path}", file=sys.stderr)
        with open(output_path, 'w', encoding='utf-8') as f:
            f.write(cleaned)


if __name__ == '__main__':
    main()
