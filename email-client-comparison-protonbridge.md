# Email Client Comparison: Agent CLI Interface via Proton Bridge (IMAP/SMTP)

**Context:** Proton Bridge exposes a local IMAP (127.0.0.1:1143) and SMTP (127.0.0.1:1025) interface with plain-auth credentials. Any standard IMAP/SMTP client works against it. The evaluation criteria are: extensibility, overhead, and agent/MCP integration surface.

---

## Candidates at a Glance

| | aerc | NeoMutt | Alpine | Mutt (classic) |
|---|---|---|---|---|
| Language | Go | C | C | C |
| Active dev | Yes (2026 releases) | Yes (2026-05-04) | Slow (1 maintainer) | Effectively dead |
| IMAP native | Yes, async | Yes | Yes | Yes |
| SMTP native | Yes | Yes (via sendmail or SMTP) | Yes | Mostly sendmail |
| Batch/headless | Partial (ex-commands, hooks) | Yes (`-s`, stdin, `-H` draft) | Limited | Yes |
| Scripting | Shell hooks, filters | muttrc + shell + Lua (experimental) | None | Shell only |
| MCP integration surface | Medium-high | Medium | Low | Low |
| Overhead | Low (single Go binary) | Very low (C) | Very low (C) | Very low (C) |

---

## Detailed Assessment

### aerc
**Language:** Go | **Actively maintained** (FOSDEM 2025 talk, releases into 2026)

aerc is the only candidate designed with a Unix-philosophy extensibility mindset from the ground up. It exposes an ex-command system (`:send-message`, `:reply`, `:flag`, `:execute`, etc.) that can be driven from shell scripts via `aerc -e 'command'`-style invocation and hooks. Its async IMAP implementation avoids the notorious network-stall problem that plagues mutt derivatives.

**Proton Bridge fit:** Trivially configured — just point `source` at `imap+insecure://user@127.0.0.1:1143` and `outgoing` at `smtp+insecure://user@127.0.0.1:1025` in `accounts.conf`. Plain auth against localhost, no TLS needed.

**Agent integration strengths:**
- Shell hooks on `new-mail`, `message-received`, etc. fire arbitrary commands — the natural MCP tool trigger surface
- Filters pipeline: pipe message content through any executable (your Rust binary, a Python script, whatever)
- The ex-command system is scriptable enough that a thin wrapper can expose `send`, `fetch`, `search` as MCP tools without touching aerc's internals
- Single static binary (Go), no runtime dependency hell on macOS or Linux

**Weaknesses:**
- Not truly headless — it's a TUI with a scripting layer, not a daemon with a protocol surface. Running it non-interactively is achievable but not its native mode.
- Go binary; doesn't integrate cleanly into a Rust Cargo workspace as a library
- The hook/filter system requires a running aerc process, which complicates agent lifecycle management if you want stateless invocations

---

### NeoMutt
**Language:** C | **Actively maintained** (released 2026-05-04)

NeoMutt has the deepest CLI/batch mode story of the group. `neomutt -s "subject" recipient < body.txt` sends headlessly. `-H draft_file` sends a pre-composed RFC822 message. These are first-class, documented, production-grade features — not hacks.

**Batch mode limitations:**
- Crypto (PGP/SMIME) is disabled in batch mode by default (requires `-C` flag). Not a concern through Proton Bridge since Bridge handles crypto transparently.
- Reading/fetching mail in batch mode is underdeveloped relative to sending. You can open a mailbox and execute muttrc commands, but it's awkward to pipe fetched messages to stdout cleanly.

**Scripting story:**
- The muttrc language is a weak DSL — no arrays, no types, external shell escapes for anything non-trivial
- Lua scripting exists but is experimental, requires compile-time flag, and the API surface is tiny (`mutt.get`, `mutt.set`, `mutt.enter`, `mutt.call`)
- The `--batch` (`-B`) flag starts NeoMutt, executes neomuttrc commands, and exits without launching ncurses — this is the most useful surface for agent integration
- Shell macro system (`push`, `exec`) is the primary automation mechanism; workable but verbose

**MCP integration:** You could wrap NeoMutt's batch send in an MCP tool and use a sidecar like `mbsync` or `offlineimap` + `notmuch` for the read/search side. This is the most battle-tested pattern in the ecosystem.

---

### Alpine (v2.26)
**Language:** C | **Maintained by one person** (Eduardo Chappa)

Alpine descends from Pine (University of Washington). It's functional, stable, and extremely conservative. Version 2.26 adds IMAP/SMTP support, XOAUTH2, password file encryption.

**For agent use: wrong tool.** Alpine has essentially no scripting or headless capability. It's a menu-driven interactive client that predates the automation-first philosophy. There is no batch mode, no hook system, no ex-command surface. Integration with an MCP layer would require screen-scraping or expect scripting — both are architectural atrocities. The single-maintainer project velocity is also a liability for a production infrastructure dependency.

**Verdict: eliminate.**

---

### Mutt (classic, via tecmint link)
The tecmint link describes `mutt -s subject recipient < body` — the classic send-only batch invocation. Original mutt is effectively abandonware at this point (last meaningful release 2019). NeoMutt supersedes it in every dimension. Unless there's a specific legacy reason to run vanilla mutt, it shouldn't be on the list.

**Verdict: eliminate, use NeoMutt if you're going the mutt route.**

---

## Head-to-Head: aerc vs. NeoMutt for Agent Use

| Criterion | aerc | NeoMutt |
|---|---|---|
| Send headlessly | Achievable via hooks/ex-commands | Native batch mode (`-s`, `-H`, stdin) |
| Fetch/read headlessly | Awkward; TUI-first design | Awkward; use mbsync+notmuch sidecar |
| Hook/trigger surface | Rich (`new-mail`, filters, ex-commands) | muttrc macros + shell escapes |
| Extensibility ceiling | Higher (async, filter pipeline, modern design) | Lower (DSL limits, C codebase) |
| Overhead | ~30MB Go binary, minimal runtime | ~5MB C binary, minimal runtime |
| MCP tool wrapping | Thin shell wrapper around ex-commands or hooks | Thin shell wrapper around batch mode |
| Proton Bridge config | Clean, well-documented | Clean, well-documented |
| macOS support | Homebrew available; notmuch requires source build | Homebrew available |
| Long-term trajectory | Actively growing feature set | Stable, not shrinking |

**Recommendation: aerc**, specifically because its filter pipeline and hook system give you a cleaner MCP integration surface without requiring a sidecar mail sync daemon. The async IMAP is also a better fit for an always-on agent context.

For an agent that primarily **sends** and does light **reading**, NeoMutt's batch mode is actually simpler to wire up and has zero ambiguity. If the agent workflow is mostly outbound (Wheelhouse-style task output → email notification), NeoMutt batch send + mbsync for ingestion is a legitimate lower-complexity option.

---

## The Rust Custom Client Option

### What the ecosystem currently looks like

- **`async-imap`** (chatmail fork of jonhoo/rust-imap): Async IMAP client, Tokio-based, actively maintained. Clean API — connect, authenticate, SELECT, FETCH, SEARCH, IDLE.
- **`lettre`**: The standard Rust SMTP library. Mature, async (Tokio), solid ergonomics. Build an `SmtpTransport`, send a `Message`. Proton Bridge is just `SmtpTransport::builder_dangerous("127.0.0.1").port(1025)` with plain credentials.
- **`mail-parser`** / **`mail-builder`**: For parsing RFC822/MIME bodies and constructing outgoing messages.

A minimal agent-grade Rust email client for your use case is **not a large project** — probably 800-1500 lines for a tool that can fetch unread messages, send a message, and search by subject/sender. The IMAP wire protocol is verbose but the `async-imap` crate abstracts the painful parts.

### Pros of rolling your own

1. **Native Panorama workspace integration** — it's a crate, not a subprocess. Call it directly from Wheelhouse, Orchestrator, wherever. No shell escape overhead, no process spawning per operation.
2. **Exact interface you need** — design the API surface around `AgentBrief` retrieval, `OutputContract` delivery via email, whatever fits the architecture. No impedance mismatch.
3. **Control plane integration** — Cloak token lifecycle, CCEE event emission, structured logging into the audit trail all happen in-process without IPC friction.
4. **No TUI overhead** — aerc and mutt carry significant code surface dedicated to terminal rendering you will never use in an agent context.
5. **Single binary** — the email capability ships inside the Panorama workspace binary, not as an external dependency with its own config directory and version skew risk.
6. **MCP server** — wrapping your Rust client as an MCP server is ~200 lines of Axum. You own the tool schema, the error surface, the retry behavior. No glue layer.
7. **Proton Bridge is dead simple** — plain IMAP/SMTP to localhost, no OAuth dance, no token refresh. The complexity that makes external clients annoying to configure doesn't exist here.

### Cons of rolling your own

1. **Email is a tar pit** — RFC822, MIME multipart, encoded headers, quoted-printable, HTML bodies, attachments. You will encounter edge cases in real Proton message formatting that `async-imap` + `mail-parser` handle for you but that will cost you debugging time when they surface.
2. **IMAP is stateful and tricky** — IDLE, connection loss handling, UID vs sequence number consistency, EXPUNGE mid-session. `async-imap` handles the wire protocol but you own the session management logic.
3. **Zero ecosystem around it** — if you want notmuch-style search indexing later, you either build it or add an external dependency anyway.
4. **Upfront time investment** — aerc or NeoMutt gets you functional in an afternoon. A solid Rust client that handles the real-world Proton Bridge message surface correctly takes days to weeks.
5. **MIME parsing is genuinely ugly** — `mail-parser` is good but Proton messages can contain S/MIME-adjacent structures from the Bridge's own encryption layer. Test thoroughly.

### The honest verdict on custom Rust

**Build it**, with one caveat: don't build a feature-complete email client. Build a **purpose-specific email transport crate** (`panorama-mail` or similar) that exposes exactly the operations your agent needs:

```rust
// The interface you actually need
pub trait AgentMailTransport {
    async fn send(&self, to: &str, subject: &str, body: &str) -> Result<(), MailError>;
    async fn fetch_unread(&self) -> Result<Vec<AgentMessage>, MailError>;
    async fn search(&self, query: &str) -> Result<Vec<AgentMessage>, MailError>;
    async fn mark_read(&self, uid: u32) -> Result<(), MailError>;
}
```

That's ~1000 lines of Rust, lives in the Panorama workspace, integrates cleanly with your existing Axum/Tokio runtime, and eliminates an entire class of external process management problems. The full-featured email client capabilities you're giving up (threading UI, HTML rendering, address books) are irrelevant for an agent interface.

---

## Summary Recommendation

| Scenario | Recommendation |
|---|---|
| Need it working this week with minimum friction | **aerc** — hooks + filter pipeline + ex-command system give the cleanest MCP wrapping surface |
| Primarily outbound (send notifications/results) | **NeoMutt batch mode** — battle-tested, trivial to script, zero ceremony |
| Medium-term Panorama workspace integration | **Custom `panorama-mail` crate** — `async-imap` + `lettre`, purpose-scoped interface, no TUI overhead |
| Long-term production agent infrastructure | **Custom crate** — the integration debt of shelling out to an external mail client compounds badly at scale |

The custom Rust path is the correct long-term answer for Wheelhouse, but aerc is a defensible short-term bridge that won't embarrass you architecturally while the mail crate gets built.
