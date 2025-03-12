# Usage: uv run md2ipynb.py input.md [output.ipynb]
#
# - Convert a Markdown file to a Jupyter Notebook.
# - The python code blocks with '```python markdown-code-runner' in the Markdown file
#   will be converted to code cells in the Jupyter Notebook.
# - The other parts of the Markdown file will be converted to markdown cells.

import nbformat
import argparse
import re
import sys
import os


def md_to_ipynb(md_file, ipynb_file):
    with open(md_file, "r", encoding="utf-8") as f:
        md_lines = f.readlines()

    nb = nbformat.v4.new_notebook()
    cell_content = []
    in_code_block = False

    for line in md_lines:
        # Python コードブロックの開始
        if re.match(r"^```python markdown-code-runner", line.strip()):
            if cell_content:
                nb.cells.append(
                    nbformat.v4.new_markdown_cell("".join(cell_content).strip())
                )
                cell_content = []
            in_code_block = True
        # コードブロックの終了
        elif line.strip() == "```" and in_code_block:
            nb.cells.append(nbformat.v4.new_code_cell("".join(cell_content).strip()))
            cell_content = []
            in_code_block = False
        # コードブロック内の行
        elif in_code_block:
            cell_content.append(line)
        # 普通のMarkdown行
        else:
            cell_content.append(line)

    # 最後のセルを追加
    if cell_content:
        nb.cells.append(nbformat.v4.new_markdown_cell("".join(cell_content).strip()))

    with open(ipynb_file, "w", encoding="utf-8") as f:
        nbformat.write(nb, f)


def main():
    parser = argparse.ArgumentParser(
        description="Convert a Markdown file to a Jupyter Notebook."
    )
    parser.add_argument("input", help="Input Markdown file")
    parser.add_argument(
        "output",
        nargs="?",
        help="Output Jupyter Notebook file (default: replace .md with .ipynb)",
    )
    args = parser.parse_args()

    if not args.input.endswith(".md"):
        print("Error: Input file must have a .md extension", file=sys.stderr)
        sys.exit(1)

    output_file = (
        args.output if args.output else os.path.splitext(args.input)[0] + ".ipynb"
    )
    md_to_ipynb(args.input, output_file)


if __name__ == "__main__":
    main()
