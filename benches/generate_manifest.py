#!/usr/bin/env python3
"""Generate manifest.json for benchmark corpus files"""

import hashlib
import json
from pathlib import Path

CORPUS_DIR = Path("benches/corpora")

def sha256_file(filepath):
    """Calculate SHA-256 hash of a file"""
    sha256 = hashlib.sha256()
    with open(filepath, 'rb') as f:
        while chunk := f.read(8192):
            sha256.update(chunk)
    return sha256.hexdigest()

def count_events(content):
    """Estimate event count by parsing the markdown content.
    This is a rough estimate based on line count and special markers."""
    lines = content.split('\n')
    events = 0

    in_code_block = False
    in_table = False

    for line in lines:
        # Code blocks
        if line.strip().startswith('```'):
            events += 2  # Start/End tag
            in_code_block = not in_code_block
            continue

        if in_code_block:
            events += 1  # Text event
            continue

        # Headings
        if line.startswith('#'):
            events += 3  # Start tag, text, end tag

        # Tables
        if '|' in line:
            if not in_table:
                events += 1  # Table start
                in_table = True
            events += 2  # Row
        elif in_table:
            events += 1  # Table end
            in_table = False

        # Math
        events += line.count('$')  # Rough estimate
        events += line.count('$$') * 2

        # Footnotes
        if '[^' in line:
            events += 2

        # Strikethrough
        if '~~' in line:
            events += line.count('~~') // 2 * 2

        # Metadata blocks (YAML front matter)
        if line.strip() == '---':
            events += 2

        # Regular text
        if line.strip():
            events += 1

    return max(events, len(lines))  # At least one event per line

def count_features(content):
    """Count specific feature occurrences in markdown"""
    return {
        "code_blocks": content.count('```') // 2,
        "inline_math": content.count('$') - content.count('$$') * 2,
        "display_math": content.count('$$') // 2,
        "tables": content.count('|---'),  # Table headers
        "footnotes": content.count('[^'),
        "metadata_blocks": content.count('---\n') // 2,
        "strikethrough": content.count('~~') // 2,
    }

def classify_family(path):
    """Classify corpus file into a family"""
    parent = path.parent.name
    return parent

def process_file(filepath):
    """Process a single corpus file and return its metadata"""
    with open(filepath, 'r') as f:
        content = f.read()

    size = filepath.stat().st_size
    sha256 = sha256_file(filepath)
    family = classify_family(filepath)

    relative_path = filepath.relative_to(CORPUS_DIR)

    features = count_features(content)
    event_count = count_events(content)

    return {
        "path": str(relative_path),
        "size_bytes": size,
        "sha256": sha256,
        "family": family,
        "event_count": event_count,
        **features
    }

def generate_manifest():
    """Generate the complete manifest"""
    manifest = {
        "version": "1.0",
        "description": "Benchmark corpus manifest with cryptographic hashes and metadata",
        "files": []
    }

    # Find all corpus files
    for family_dir in CORPUS_DIR.iterdir():
        if not family_dir.is_dir() or family_dir.name.startswith('.'):
            continue

        for corpus_file in family_dir.glob('*.*'):
            if corpus_file.is_file() and not corpus_file.name.startswith('.'):
                metadata = process_file(corpus_file)
                manifest["files"].append(metadata)

    # Sort by path for consistency
    manifest["files"].sort(key=lambda x: x["path"])

    # Write manifest
    manifest_path = CORPUS_DIR / "manifest.json"
    with open(manifest_path, 'w') as f:
        json.dump(manifest, f, indent=2)

    print(f"Generated manifest with {len(manifest['files'])} files")
    print(f"Written to: {manifest_path}")

    # Print summary
    print("\nManifest summary:")
    families = {}
    for file in manifest["files"]:
        family = file["family"]
        if family not in families:
            families[family] = []
        families[family].append(file["path"])

    for family, files in sorted(families.items()):
        print(f"  {family}: {len(files)} files")
        for file in sorted(files):
            print(f"    - {file}")

if __name__ == "__main__":
    generate_manifest()
