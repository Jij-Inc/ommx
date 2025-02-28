#!/usr/bin/env python3
"""
Script to generate a consolidated markdown file for AI assistants.

This script:
1. Converts Jupyter notebooks to markdown
2. Concatenates markdown files based on TOC order
3. Generates Python stub files for the ommx package
4. Combines tutorials and API stubs into a single LLMs.txt file
"""

import os
import sys
import yaml
import subprocess
import tempfile
import shutil
from pathlib import Path


def run_command(cmd, cwd=None):
    """Run a shell command and return its output."""
    print(f"Running: {' '.join(cmd)}")
    try:
        result = subprocess.run(
            cmd, 
            cwd=cwd, 
            check=True, 
            stdout=subprocess.PIPE, 
            stderr=subprocess.PIPE,
            text=True
        )
        return result.stdout
    except subprocess.CalledProcessError as e:
        print(f"Error executing command: {' '.join(cmd)}")
        print(f"Error output: {e.stderr}")
        sys.exit(1)

def install_package(package_name):
    """Install a Python package using uv add."""
    cmd = ["uv", "add", "--dev", package_name]
    print(f"Installing package: {package_name}")
    return run_command(cmd)


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
        output_path = Path(output_dir) / filename.replace('.ipynb', '.md')
        
        # Run nbconvert
        cmd = [
            "uv", "run", "jupyter", "nbconvert", 
            "--to", "markdown", 
            "--output", str(output_path),
            str(notebook_path)
        ]
        run_command(cmd)
        
        print(f"Converted {notebook_path} to {output_path}")


def read_toc_file(toc_path):
    """Read the TOC file and return the ordered list of files."""
    with open(toc_path, 'r') as f:
        toc_data = yaml.safe_load(f)
    
    ordered_files = []
    
    # Extract the root file
    if 'root' in toc_data:
        ordered_files.append(f"{toc_data['root']}.md")
    
    # Extract files from parts and chapters
    if 'parts' in toc_data:
        for part in toc_data['parts']:
            if 'chapters' in part:
                for chapter in part['chapters']:
                    if 'file' in chapter:
                        ordered_files.append(f"{chapter['file']}.md")
    
    return ordered_files


def concatenate_markdown_files(docs_dir, ordered_files, output_file):
    """Concatenate markdown files in the order specified by the TOC."""
    with open(output_file, 'w') as outfile:
        outfile.write("# OMMX Documentation for AI Assistants\n\n")
        outfile.write("## Tutorial Content\n\n")
        
        # Get list of markdown files in the directory
        markdown_files = [f for f in os.listdir(docs_dir) if f.endswith('.md')]
        
        # Process files in TOC order
        for file_path in ordered_files:
            # Skip files that are not tutorials
            if not file_path.startswith('tutorial/'):
                continue
                
            # Extract the base filename
            base_name = os.path.basename(file_path)
            
            # Find the corresponding markdown file
            matching_file = None
            for md_file in markdown_files:
                if md_file.startswith(base_name):
                    matching_file = md_file
                    break
            
            if not matching_file:
                print(f"Warning: No matching file for {base_name} found in {docs_dir}, skipping")
                continue
            
            full_path = os.path.join(docs_dir, matching_file)
            
            # Add a section header for the file
            section_name = base_name.replace('_', ' ').title()
            outfile.write(f"### {section_name}\n\n")
            
            # Append the file content
            with open(full_path, 'r') as infile:
                content = infile.read()
                # Remove the first heading (it's already in the section header)
                lines = content.split('\n')
                if lines and lines[0].startswith('# '):
                    content = '\n'.join(lines[1:])
                outfile.write(content)
                outfile.write('\n\n')


def generate_python_stubs(output_dir):
    """Generate Python stub files for the ommx package."""
    os.makedirs(output_dir, exist_ok=True)
    
    # Run pyright to generate stubs
    cmd = ["uv", "run", "pyright", "--createstub", "ommx"]
    run_command(cmd)
    
    # Check if stubs were generated
    stub_dir = "./typings/ommx"
    if not os.path.exists(stub_dir):
        print("Error: Stub files were not generated")
        return False
    
    # Copy stub files to output directory
    for stub_file in Path(stub_dir).glob("**/*.pyi"):
        relative_path = stub_file.relative_to(stub_dir)
        dest_path = Path(output_dir) / relative_path
        
        # Create output directory if it doesn't exist
        os.makedirs(dest_path.parent, exist_ok=True)
        
        # Copy the file
        shutil.copy2(stub_file, dest_path)
        print(f"Copied {stub_file} to {dest_path}")
    
    return True


def concatenate_api_stubs(stubs_dir, output_file):
    """Concatenate API stub files into the output file."""
    with open(output_file, 'a') as outfile:
        outfile.write("## API Reference\n\n")
        
        # Find all stub files
        stub_files = list(Path(stubs_dir).glob("**/*.pyi"))
        
        if not stub_files:
            outfile.write("No API stub files found.\n\n")
            return
        
        # Process each stub file
        for stub_file in sorted(stub_files):
            # Add a section header for the file
            module_path = stub_file.relative_to(stubs_dir).with_suffix('')
            module_name = str(module_path).replace('/', '.')
            outfile.write(f"### {module_name}\n\n")
            
            # Append the file content
            with open(stub_file, 'r') as infile:
                content = infile.read()
                outfile.write("```python\n")
                outfile.write(content)
                outfile.write("\n```\n\n")


def main():
    """Main function to generate the LLMs.txt file."""
    # Define paths
    repo_root = Path(__file__).parent.parent
    docs_dir = repo_root / "docs" / "en"
    notebook_dir = docs_dir / "tutorial"
    toc_path = docs_dir / "_toc.yml"
    output_file = repo_root / "LLMs.txt"
    
    # Ensure required packages are installed
    required_packages = ["pyyaml", "jupyter", "nbconvert", "pyright"]
    for package in required_packages:
        install_package(package)
    
    # Create temporary directories
    with tempfile.TemporaryDirectory() as temp_dir:
        markdown_dir = Path(temp_dir) / "markdown"
        stubs_dir = Path(temp_dir) / "stubs"
        
        # Convert notebooks to markdown
        convert_notebooks_to_markdown(notebook_dir, markdown_dir)
        
        # Read TOC file
        ordered_files = read_toc_file(toc_path)
        
        # Concatenate markdown files
        concatenate_markdown_files(markdown_dir, ordered_files, output_file)
        
        # Generate Python stubs
        if generate_python_stubs(stubs_dir):
            # Concatenate API stubs
            concatenate_api_stubs(stubs_dir, output_file)
        
    print(f"Generated {output_file}")


if __name__ == "__main__":
    main()
