#!/usr/bin/env python3
"""
Script to generate a consolidated markdown file for AI assistants.

This script:
1. Converts Jupyter notebooks to markdown
2. Concatenates markdown files based on TOC order
3. Creates a single LLMs.txt file with tutorial content
"""

import os
import yaml
import tempfile
import re
import shutil
from pathlib import Path
import nbformat
from nbconvert import MarkdownExporter


def convert_notebook_to_markdown(notebook_path, output_path):
    """Convert a Jupyter notebook to markdown using nbconvert module."""
    print(f"Converting {notebook_path} to {output_path}")

    # Read the notebook
    with open(notebook_path, "r", encoding="utf-8") as f:
        nb = nbformat.read(f, as_version=4)

    # Initialize MarkdownExporter
    exporter = MarkdownExporter()

    # Convert notebook to markdown
    body, _ = exporter.from_notebook_node(nb)

    # Write the output file
    with open(output_path, "w", encoding="utf-8") as f:
        f.write(body)


def convert_notebooks_to_markdown(notebook_dir, output_dir):
    """Convert Jupyter notebooks to markdown files."""
    os.makedirs(output_dir, exist_ok=True)

    # Find all notebook files
    notebook_files = list(Path(notebook_dir).glob("**/*.ipynb"))

    if not notebook_files:
        print(f"No notebook files found in {notebook_dir}")
        return

    print(f"Found {len(notebook_files)} notebook files")

    # Convert each notebook to markdown
    for notebook_path in notebook_files:
        # Get the filename without directory
        filename = notebook_path.name
        output_path = Path(output_dir) / filename.replace(".ipynb", ".md")

        # Convert notebook to markdown
        convert_notebook_to_markdown(notebook_path, output_path)


def read_toc_file(toc_path):
    """Read the TOC file and return the ordered list of files."""
    with open(toc_path, "r") as f:
        toc_data = yaml.safe_load(f)

    ordered_files = []

    # Extract the root file
    if "root" in toc_data:
        ordered_files.append(f"{toc_data['root']}.md")

    # Extract files from parts and chapters
    if "parts" in toc_data:
        for part in toc_data["parts"]:
            if "chapters" in part:
                for chapter in part["chapters"]:
                    if "file" in chapter:
                        ordered_files.append(f"{chapter['file']}.md")

    return ordered_files


def concatenate_markdown_files(docs_dir, ordered_files, output_file):
    """Concatenate markdown files in the order specified by the TOC."""
    with open(output_file, "w") as outfile:
        outfile.write("# OMMX Documentation for AI Assistants\n\n")

        # Track current section for adding headers
        current_section = None

        # Process files in TOC order
        for file_path in ordered_files:
            # Extract the section (first directory in the path)
            if "/" in file_path:
                section = file_path.split("/")[0]
            else:
                section = "root"

            # Add section header if it's a new section
            if section != current_section:
                current_section = section
                section_title = section.replace("_", " ").title()
                if section == "root":
                    section_title = "Introduction"
                outfile.write(f"## {section_title}\n\n")

            # Extract the base filename without extension
            if file_path.endswith(".md"):
                base_name = os.path.basename(file_path[:-3])
            else:
                base_name = os.path.basename(file_path)

            # Construct the path to the markdown file in the temporary directory
            if "/" in file_path:
                # For files in subdirectories
                subdir = os.path.dirname(file_path)
                md_path = os.path.join(docs_dir, f"{base_name}.md")
                # Also try with the subdirectory
                if not os.path.exists(md_path):
                    md_path = os.path.join(docs_dir, subdir, f"{base_name}.md")
            else:
                # For files in the root directory
                md_path = os.path.join(docs_dir, f"{base_name}.md")

            # Check if the file exists
            if not os.path.exists(md_path):
                print(
                    f"Warning: No matching file for {base_name} found at {md_path}, skipping"
                )
                continue

            # Add a section header for the file
            file_title = base_name.replace("_", " ").title()
            outfile.write(f"### {file_title}\n\n")

            # Append the file content
            with open(md_path, "r") as infile:
                content = infile.read()
                # Remove the first heading (it's already in the section header)
                lines = content.split("\n")
                if lines and lines[0].startswith("# "):
                    content = "\n".join(lines[1:])

                # Exclude images and div elements
                # Remove image markdown (```{figure} ... ```)
                content = re.sub(r"```\{figure\}.*?```", "", content, flags=re.DOTALL)
                # Remove inline images (![...](...)
                content = re.sub(r"!\[.*?\]\(.*?\)", "", content)
                # Remove div elements (<div>...</div>)
                content = re.sub(r"<div.*?>.*?</div>", "", content, flags=re.DOTALL)

                outfile.write(content)
                outfile.write("\n\n")


def main():
    """Main function to generate the LLMs.txt file."""
    # Define paths
    repo_root = Path(__file__).parent.parent
    docs_dir = repo_root / "docs" / "en"
    notebook_dir = docs_dir  # Process all notebooks in docs/en, not just tutorial
    toc_path = docs_dir / "_toc.yml"
    output_file = repo_root / "LLMs.txt"

    # Create temporary directories
    with tempfile.TemporaryDirectory() as temp_dir:
        markdown_dir = Path(temp_dir) / "markdown"

        # Convert notebooks to markdown
        convert_notebooks_to_markdown(notebook_dir, markdown_dir)

        # Copy existing markdown files to the temporary directory
        for md_file in docs_dir.glob("**/*.md"):
            relative_path = md_file.relative_to(docs_dir)
            target_path = markdown_dir / relative_path
            os.makedirs(target_path.parent, exist_ok=True)
            shutil.copy(md_file, target_path)

        # Read TOC file
        ordered_files = read_toc_file(toc_path)

        # Concatenate markdown files
        concatenate_markdown_files(markdown_dir, ordered_files, output_file)

    print(f"Generated {output_file}")


if __name__ == "__main__":
    main()
