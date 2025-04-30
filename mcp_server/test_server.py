"""
Tests for the OMMX Documentation MCP Server.
"""

import os
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))

from mcp_server.server import (
    get_document_content,
    list_documents_by_language_and_category,
    search_documents,
)


class TestOMMXDocumentationMCPServer(unittest.TestCase):
    """Test cases for the OMMX Documentation MCP Server."""

    def test_get_document_content(self):
        """Test retrieving document content."""
        content = get_document_content("en", "introduction")
        self.assertIsInstance(content, str)
        self.assertGreater(len(content), 0)
        self.assertIn("OMMX", content)

    def test_list_documents_by_language(self):
        """Test listing documents by language."""
        documents = list_documents_by_language_and_category("en")
        self.assertIsInstance(documents, list)
        self.assertGreater(len(documents), 0)
        self.assertTrue(any("introduction.md" in doc for doc in documents))

    def test_list_documents_by_category(self):
        """Test listing documents by category."""
        documents = list_documents_by_language_and_category("en", "tutorial")
        self.assertIsInstance(documents, list)
        self.assertGreater(len(documents), 0)
        self.assertTrue(any("tutorial" in doc for doc in documents))

    def test_search_documents(self):
        """Test searching documents."""
        results = search_documents("en", "OMMX")
        self.assertIsInstance(results, list)
        self.assertGreater(len(results), 0)
        for result in results:
            self.assertIn("path", result)
            self.assertIn("snippet", result)
            self.assertTrue("ommx" in result["snippet"].lower())


if __name__ == "__main__":
    unittest.main()
