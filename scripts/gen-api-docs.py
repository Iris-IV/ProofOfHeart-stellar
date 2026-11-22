#!/usr/bin/env python3
"""Generate docs/API.md (the entrypoint reference) from the contract source (#1465).

Parses the `#[contractimpl]` block in src/lib.rs: every `pub fn` is a public
entrypoint. Usage:
    python3 scripts/gen-api-docs.py           # (re)write docs/API.md
    python3 scripts/gen-api-docs.py --check   # fail if docs/API.md is stale or
                                              # misses an entrypoint (CI)
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "src" / "lib.rs"
OUT = ROOT / "docs" / "API.md"
AUTH_DOC = ROOT / "docs" / "AUTHORIZATION.md"

ADDR_ARGS = ["admin", "creator", "contributor", "voter", "delegator", "delegate",
             "caller", "owner", "new_owner", "recipient", "donor", "new_admin"]


def parse():
    text = SRC.read_text()
    start = text.index("#[contractimpl]")
    lines = text[start:].splitlines()
    out, doc, i = [], [], 0
    while i < len(lines):
        s = lines[i].strip()
        if s.startswith("///"):
            doc.append(s[3:][1:] if s.startswith("/// ") else s[3:])
        elif s.startswith("#["):
            pass
        elif s.startswith("pub fn "):
            sig = s
            while "{" not in sig:
                i += 1
                sig += " " + lines[i].strip()
            body = []
            j = i + 1
            while j < len(lines) and lines[j] != "    }":
                body.append(lines[j])
                j += 1
            m = re.match(r"pub fn (\w+)\((.*?)\)\s*(?:->\s*(.*?))?\s*\{", sig)
            name, args, ret = m.group(1), m.group(2), (m.group(3) or "()")
            args = [a.strip() for a in re.split(r",\s*(?![^<]*>)", args) if a.strip()]
            args = [a for a in args if not re.match(r"_?env\s*:", a)]
            out.append((name, args, ret.strip(), doc, "\n".join(body)))
            doc, i = [], j
        elif s and not s.startswith("//"):
            doc = []
        elif s.startswith("//"):
            doc = []
        i += 1
    return out


def auth_table():
    table = {}
    if AUTH_DOC.exists():
        for m in re.finditer(r"^\| `(\w+)\(.*?\)` \| (.+?) \|\s*$", AUTH_DOC.read_text(), re.M):
            table[m.group(1)] = m.group(2).strip()
    return table


def infer_auth(name, args, body, table):
    if name in table:
        return table[name] + " (per [AUTHORIZATION.md](AUTHORIZATION.md))"
    names = [a.split(":")[0].strip() for a in args]
    if "assert_admin" in body or "require_admin" in body:
        who = next((n for n in names if n in ("admin", "caller")), None)
        return f"`{who}` (must be the stored admin)" if who else "stored admin"
    for n in ADDR_ARGS:
        if n in names and n not in ("admin",):
            return f"`{n}`"
    if "require_auth" in body:
        return "see implementation (`require_auth`)"
    if not name.startswith(("set_", "update_", "add_")) and "env.storage().persistent().set" not in body and "Result<" not in "":
        pass
    return "none (read-only or permissionless)" if name.startswith(("get_", "is_", "has_", "list_")) else "see [AUTHORIZATION.md](AUTHORIZATION.md)"


def errors_of(doc):
    errs, on = [], False
    for l in doc:
        if l.strip() == "# Errors":
            on = True
        elif on and l.strip().startswith("*"):
            errs.append(l.strip().lstrip("* ").strip())
        elif on and l.strip().startswith("#"):
            on = False
    return errs


def render():
    eps = parse()
    table = auth_table()
    o = ["# Contract API Reference", "",
         "> **Generated** by `python3 scripts/gen-api-docs.py` from `src/lib.rs`. Do not edit by hand; "
         "CI (`--check`) fails when this file is stale or misses an entrypoint (#1465).", "",
         f"The contract exposes **{len(eps)}** public entrypoints. Errors are the `Error` codes "
         "documented in `src/errors.rs`; authorization follows [AUTHORIZATION.md](AUTHORIZATION.md).", "",
         "## Index", ""]
    o.append(", ".join(f"[`{e[0]}`](#{e[0]})" for e in eps))
    o.append("")
    o.append("## Entrypoints")
    for name, args, ret, doc, body in eps:
        o += ["", f"### {name}", "", "```rust", f"{name}({', '.join(args)}) -> {ret}", "```", ""]
        summary = []
        for l in doc:
            if l.strip().startswith("# "):
                break
            summary.append(l)
        o.append("\n".join(summary).strip() or "_No description._")
        o += ["", f"- **Auth:** {infer_auth(name, args, body, table)}"]
        errs = errors_of(doc)
        if errs:
            o.append("- **Errors:**")
            o += [f"  - {e}" for e in errs]
        elif "Result<" in ret:
            o.append("- **Errors:** returns `Result<_, Error>`; see `src/errors.rs` for the full code list.")
        else:
            o.append("- **Errors:** none (infallible).")
    return "\n".join(o) + "\n"


def main():
    new = render()
    if "--check" in sys.argv:
        names = [e[0] for e in parse()]
        old = OUT.read_text() if OUT.exists() else ""
        missing = [n for n in names if f"### {n}\n" not in old]
        if missing:
            print("docs/API.md is missing entrypoint(s): " + ", ".join(missing), file=sys.stderr)
        if old != new:
            print("docs/API.md is out of date. Run: python3 scripts/gen-api-docs.py", file=sys.stderr)
            sys.exit(1)
        print(f"docs/API.md covers all {len(names)} entrypoints.")
        return
    OUT.write_text(new)
    print(f"Wrote {OUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
