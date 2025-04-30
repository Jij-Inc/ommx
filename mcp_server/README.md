# OMMX Documentation MCP Server

This MCP (Model Context Protocol) server provides access to OMMX documentation through resources and tools. It allows LLMs to access and search through OMMX documentation in both English and Japanese.

<details>
<summary>日本語</summary>

# OMMXドキュメントMCPサーバー

このMCP（Model Context Protocol）サーバーは、リソースとツールを通じてOMMXドキュメントへのアクセスを提供します。LLMが英語と日本語の両方でOMMXドキュメントにアクセスし検索することができます。
</details>

## Installation

The MCP server is included in the OMMX repository. To use it, you need to have OMMX installed:

```bash
# Clone the repository if you haven't already
git clone https://github.com/Jij-Inc/ommx.git
cd ommx

# Install dependencies
pip install -e .
```

<details>
<summary>日本語</summary>

## インストール

MCPサーバーはOMMXリポジトリに含まれています。使用するには、OMMXをインストールする必要があります：

```bash
# まだクローンしていない場合はリポジトリをクローン
git clone https://github.com/Jij-Inc/ommx.git
cd ommx

# 依存関係をインストール
pip install -e .
```
</details>

## Usage

### Starting the Server

To start the MCP server, run:

```python
from mcp_server.server import mcp

# Start the server with stdio transport
mcp.run(transport="stdio")

# Alternatively, start with HTTP transport on port 8000
# mcp.run(transport="http", port=8000)
```

<details>
<summary>日本語</summary>

## 使用方法

### サーバーの起動

MCPサーバーを起動するには、次のように実行します：

```python
from mcp_server.server import mcp

# stdioトランスポートでサーバーを起動
mcp.run(transport="stdio")

# または、HTTPトランスポートでポート8000で起動
# mcp.run(transport="http", port=8000)
```
</details>

### Accessing Resources

The server exposes the following resources:

- `ommx://docs/{language}/{path}` - Access a specific document
- `ommx://docs/{language}/list` - List all documents in a language
- `ommx://docs/{language}/{category}/list` - List documents in a specific category

Examples:

```python
# Access the introduction document in English
resource = client.get_resource("ommx://docs/en/introduction")

# List all English documents
documents = client.get_resource("ommx://docs/en/list")

# List all tutorial documents in Japanese
tutorials = client.get_resource("ommx://docs/ja/tutorial/list")
```

<details>
<summary>日本語</summary>

### リソースへのアクセス

サーバーは以下のリソースを公開しています：

- `ommx://docs/{language}/{path}` - 特定のドキュメントにアクセス
- `ommx://docs/{language}/list` - 言語別のすべてのドキュメントを一覧表示
- `ommx://docs/{language}/{category}/list` - 特定のカテゴリのドキュメントを一覧表示

例：

```python
# 英語の導入ドキュメントにアクセス
resource = client.get_resource("ommx://docs/en/introduction")

# すべての英語ドキュメントを一覧表示
documents = client.get_resource("ommx://docs/en/list")

# 日本語のチュートリアルドキュメントをすべて一覧表示
tutorials = client.get_resource("ommx://docs/ja/tutorial/list")
```
</details>

### Using Tools

The server provides the following tools:

- `search_docs` - Search for documents matching a query
- `get_document` - Retrieve a document at a specific path
- `list_documents` - List available documents by language and category

Examples:

```python
# Search for documents containing "QUBO" in English
results = client.use_tool("search_docs", language="en", query="QUBO")

# Get a specific document
content = client.use_tool("get_document", language="en", path="tutorial/getting_started")

# List all documents in Japanese
documents = client.use_tool("list_documents", language="ja")

# List documents in a specific category
tutorials = client.use_tool("list_documents", language="en", category="tutorial")
```

<details>
<summary>日本語</summary>

### ツールの使用

サーバーは以下のツールを提供しています：

- `search_docs` - クエリに一致するドキュメントを検索
- `get_document` - 特定のパスのドキュメントを取得
- `list_documents` - 言語とカテゴリ別に利用可能なドキュメントを一覧表示

例：

```python
# 英語で「QUBO」を含むドキュメントを検索
results = client.use_tool("search_docs", language="en", query="QUBO")

# 特定のドキュメントを取得
content = client.use_tool("get_document", language="en", path="tutorial/getting_started")

# 日本語のすべてのドキュメントを一覧表示
documents = client.use_tool("list_documents", language="ja")

# 特定のカテゴリのドキュメントを一覧表示
tutorials = client.use_tool("list_documents", language="en", category="tutorial")
```
</details>

### Using Prompts

The server provides an `explore_docs` prompt to assist with exploring OMMX documentation:

```python
# Get the explore_docs prompt in English
prompt = client.get_prompt("explore_docs", language="en")

# Get the explore_docs prompt in Japanese
prompt_ja = client.get_prompt("explore_docs", language="ja")
```

<details>
<summary>日本語</summary>

### プロンプトの使用

サーバーはOMMXドキュメントの探索を支援するための`explore_docs`プロンプトを提供しています：

```python
# 英語のexplore_docsプロンプトを取得
prompt = client.get_prompt("explore_docs", language="en")

# 日本語のexplore_docsプロンプトを取得
prompt_ja = client.get_prompt("explore_docs", language="ja")
```
</details>

## Integration with Claude

To use this MCP server with Claude:

1. Start the MCP server with HTTP transport
2. Connect Claude to the server using the MCP client
3. Use Claude to explore and interact with OMMX documentation

Example conversation with Claude:

```
User: Can you help me understand how to use OMMX?

Claude: I'd be happy to help you understand OMMX. Let me access the documentation to provide you with accurate information.

[Claude accesses OMMX documentation through the MCP server]

Based on the documentation, OMMX is a framework for mathematical optimization. Here's how you can get started:

1. Installation: ...
2. Basic usage: ...
3. Advanced features: ...

Would you like me to explain any specific aspect of OMMX in more detail?
```

<details>
<summary>日本語</summary>

## Claudeとの統合

このMCPサーバーをClaudeで使用するには：

1. HTTPトランスポートでMCPサーバーを起動
2. MCPクライアントを使用してClaudeをサーバーに接続
3. Claudeを使用してOMMXドキュメントを探索し、操作する

Claudeとの会話例：

```
ユーザー：OMMXの使い方を教えてもらえますか？

Claude：OMMXの理解をお手伝いします。正確な情報を提供するためにドキュメントにアクセスしますね。

[ClaudeがMCPサーバーを通じてOMMXドキュメントにアクセス]

ドキュメントによると、OMMXは数理最適化のためのフレームワークです。以下は始め方です：

1. インストール：...
2. 基本的な使用方法：...
3. 高度な機能：...

OMMXの特定の側面についてさらに詳しく説明しましょうか？
```
</details>

## Security Considerations

The MCP server includes security measures to prevent directory traversal attacks and unauthorized access to files outside the documentation directory. It validates all file paths before accessing them and restricts access to the OMMX documentation directory only.

<details>
<summary>日本語</summary>

## セキュリティに関する考慮事項

MCPサーバーには、ディレクトリトラバーサル攻撃を防止し、ドキュメントディレクトリ外のファイルへの不正アクセスを防ぐセキュリティ対策が含まれています。すべてのファイルパスにアクセスする前に検証し、OMMXドキュメントディレクトリへのアクセスのみに制限しています。
</details>
