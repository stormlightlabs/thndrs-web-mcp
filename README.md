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
