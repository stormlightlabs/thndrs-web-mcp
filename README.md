# Thunderus Web MCP Server (`thndrs-web-mcp`)

A local-first MCP server to give Thunderus a fast, reliable web search & deterministic "reader-mode" docs ingestion, with a durable SQLite cache and strict safety controls.

## Features/Constraints

- Transport: stdio first
    - MCP defines stdio + Streamable HTTP; clients should support stdio whenever
      possible.
- Search provider: Brave Search API
- Cache: SQLite, WAL mode, content-addressed by URL+headers hash.
- Output format: Markdown for extracted docs (LLM-friendly).
- Safety: SSRF protections, robots.txt respect, rate limits, and size caps.

```mermaid
sequenceDiagram
    participant Client as MCP Client (Claude, etc.)
    participant Server as mcp-web (stdio)
    participant Lectito as lectito-core

    Client->>Server: tools/call { name: "web_extract", arguments: {...} }
    Server->>Lectito: Document::parse(html)
    Lectito-->>Server: Document
    Server->>Lectito: extract_content(&doc, &config)
    Lectito-->>Server: ExtractionResult
    Server->>Lectito: convert_to_markdown(&content, &metadata, &md_config)
    Lectito-->>Server: String (Markdown)
    Server-->>Client: CallToolResult { content: [...] }
```
