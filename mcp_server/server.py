"""
OMMX Documentation MCP Server

This server provides access to OMMX documentation through MCP resources and tools.
It exposes documentation files in both English and Japanese from the OMMX repository.
"""

import os
import glob
from pathlib import Path
from typing import List, Dict, Optional

from fastmcp import FastMCP

DOCS_DIR = Path(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))) / "docs"

mcp = FastMCP(
    "OMMX Documentation Server",
    instructions="""

This server provides access to OMMX documentation in both English and Japanese.

- Documentation files: `ommx://docs/{language}/{path}`
- Document listings: `ommx://docs/{language}/list` or `ommx://docs/{language}/{category}/list`

- `search_docs`: Search for documents matching a query
- `get_document`: Retrieve a document at a specific path
- `list_documents`: List available documents by language and category

- English (en)
- Japanese (ja)

- introduction: Overview of OMMX
- tutorial: Tutorials on how to use OMMX
- user_guide: Detailed user guide for OMMX
- release_note: Release notes for OMMX
""",
)


def get_document_content(language: str, path: str) -> str:
    """
    Get the content of a document.

    Args:
        language: The language of the document (en/ja)
        path: The path to the document relative to the language directory

    Returns:
        The content of the document as a string
    """
    if language not in ["en", "ja"]:
        raise ValueError(f"Invalid language: {language}. Must be 'en' or 'ja'.")

    if path.endswith(".md") or path.endswith(".ipynb"):
        full_path = DOCS_DIR / language / path
    else:
        md_path = DOCS_DIR / language / f"{path}.md"
        ipynb_path = DOCS_DIR / language / f"{path}.ipynb"

        if md_path.exists():
            full_path = md_path
        elif ipynb_path.exists():
            full_path = ipynb_path
        else:
            raise FileNotFoundError(f"Document not found: {path}")

    if not full_path.is_relative_to(DOCS_DIR):
        raise ValueError("Invalid document path: attempted directory traversal")

    if not full_path.exists():
        raise FileNotFoundError(f"Document not found: {full_path}")

    with open(full_path, "r", encoding="utf-8") as f:
        content = f.read()

    return content


def list_documents_by_language_and_category(
    language: str, category: Optional[str] = None
) -> List[str]:
    """
    List documents by language and category.

    Args:
        language: The language of the documents (en/ja)
        category: The category of the documents (optional)

    Returns:
        A list of document paths
    """
    if language not in ["en", "ja"]:
        raise ValueError(f"Invalid language: {language}. Must be 'en' or 'ja'.")

    if category:
        pattern = str(DOCS_DIR / language / category / "**" / "*.*")
    else:
        pattern = str(DOCS_DIR / language / "**" / "*.*")

    files = glob.glob(pattern, recursive=True)

    files = [f for f in files if f.endswith(".md") or f.endswith(".ipynb")]

    base_path = str(DOCS_DIR / language)
    relative_paths = [os.path.relpath(f, base_path) for f in files]

    return relative_paths


def search_documents(language: str, query: str) -> List[Dict[str, str]]:
    """
    Search for documents matching a query.

    Args:
        language: The language of the documents (en/ja)
        query: The search query

    Returns:
        A list of dictionaries containing document paths and snippets
    """
    if language not in ["en", "ja"]:
        raise ValueError(f"Invalid language: {language}. Must be 'en' or 'ja'.")

    documents = list_documents_by_language_and_category(language)

    results = []
    for doc_path in documents:
        try:
            content = get_document_content(language, doc_path)

            if query.lower() in content.lower():
                index = content.lower().find(query.lower())
                start = max(0, index - 100)
                end = min(len(content), index + len(query) + 100)
                snippet = content[start:end]

                if start > 0:
                    snippet = "..." + snippet
                if end < len(content):
                    snippet = snippet + "..."

                results.append({"path": doc_path, "snippet": snippet})
        except Exception:
            continue

    return results


@mcp.resource("ommx://docs/{language}/{path}")
def get_doc(language: str, path: str) -> str:
    """
    Get a document by language and path.

    Args:
        language: The language of the document (en/ja)
        path: The path to the document relative to the language directory

    Returns:
        The content of the document as a string
    """
    try:
        return get_document_content(language, path)
    except Exception as e:
        return f"Error retrieving document: {str(e)}"


@mcp.resource("ommx://docs/{language}/list")
def list_docs_by_language(language: str) -> List[str]:
    """
    List all documents in a language.

    Args:
        language: The language of the documents (en/ja)

    Returns:
        A list of document paths
    """
    try:
        return list_documents_by_language_and_category(language)
    except Exception as e:
        return [f"Error listing documents: {str(e)}"]


@mcp.resource("ommx://docs/{language}/{category}/list")
def list_docs_by_category(language: str, category: str) -> List[str]:
    """
    List documents in a language and category.

    Args:
        language: The language of the documents (en/ja)
        category: The category of the documents

    Returns:
        A list of document paths
    """
    try:
        return list_documents_by_language_and_category(language, category)
    except Exception as e:
        return [f"Error listing documents: {str(e)}"]


@mcp.tool()
def search_docs(language: str, query: str) -> List[Dict[str, str]]:
    """
    Search for documents matching a query.

    Args:
        language: The language of the documents (en/ja)
        query: The search query

    Returns:
        A list of dictionaries containing document paths and snippets
    """
    return search_documents(language, query)


@mcp.tool()
def get_document(language: str, path: str) -> str:
    """
    Retrieve a document at a specific path.

    Args:
        language: The language of the document (en/ja)
        path: The path to the document relative to the language directory

    Returns:
        The content of the document as a string
    """
    try:
        return get_document_content(language, path)
    except Exception as e:
        return f"Error retrieving document: {str(e)}"


@mcp.tool()
def list_documents(language: str, category: Optional[str] = None) -> List[str]:
    """
    List available documents by language and category.

    Args:
        language: The language of the documents (en/ja)
        category: The category of the documents (optional)

    Returns:
        A list of document paths
    """
    try:
        return list_documents_by_language_and_category(language, category)
    except Exception as e:
        return [f"Error listing documents: {str(e)}"]


@mcp.prompt("explore_docs")
def explore_docs_prompt(language: str = "en") -> str:
    """
    Prompt to assist with exploring OMMX documentation.

    Args:
        language: The language to use (en/ja)

    Returns:
        A prompt string
    """
    if language == "ja":
        return """
        OMMXドキュメントを探索しましょう。以下のカテゴリから選択できます：
        - introduction: OMMXの概要
        - tutorial: OMMXの使い方チュートリアル
        - user_guide: OMMXの詳細なユーザーガイド
        - release_note: OMMXのリリースノート
        
        どのカテゴリのドキュメントに興味がありますか？
        """
    else:
        return """
        Let's explore OMMX documentation. You can choose from the following categories:
        - introduction: Overview of OMMX
        - tutorial: Tutorials on how to use OMMX
        - user_guide: Detailed user guide for OMMX
        - release_note: Release notes for OMMX
        
        Which category of documents are you interested in?
        """


if __name__ == "__main__":
    print("Starting OMMX Documentation MCP Server")
    mcp.run(transport="stdio")
