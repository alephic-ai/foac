#!/usr/bin/env python3
"""Measure the context tax foac claims to remove. See doc/context-tax.md.

Builds the repo, then measures the repo build (target/debug/foac) under a
throwaway HOME with only a dummy LINEAR_API_KEY, so exactly one provider is
visible and the numbers are reproducible on any machine. The MCP baseline is
the tools/list payload of the official GitHub MCP server, measured live from
Docker; without Docker the foac rows still print and the script exits 1.
"""

import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
FOAC = REPO / "target" / "debug" / "foac"
MCP_IMAGE = "ghcr.io/github/github-mcp-server:v1.11.0"


def foac(env, *args):
    """Run the repo-build foac with the throwaway environment, return stdout bytes."""
    r = subprocess.run([str(FOAC), *args], env=env, capture_output=True, check=True)
    return r.stdout


def frontmatter_value(skill, key):
    for line in skill.decode().splitlines():
        if line.startswith(key + ": "):
            return line[len(key) + 2 :]
    raise ValueError(f"no {key!r} in skill frontmatter")


def measure_mcp():
    """Return (wire_bytes, tool_count, server_version) for GitHub MCP tools/list."""
    proc = subprocess.Popen(
        ["docker", "run", "-i", "--rm",
         "-e", "GITHUB_PERSONAL_ACCESS_TOKEN=dummy", MCP_IMAGE, "stdio"],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
    )
    # The server needs stdin held open through the full handshake.
    def send(msg):
        proc.stdin.write((json.dumps(msg) + "\n").encode())
        proc.stdin.flush()

    send({"jsonrpc": "2.0", "id": 1, "method": "initialize",
          "params": {"protocolVersion": "2025-03-26", "capabilities": {},
                     "clientInfo": {"name": "context-tax", "version": "0"}}})
    init = json.loads(proc.stdout.readline())
    version = init["result"]["serverInfo"]["version"]
    send({"jsonrpc": "2.0", "method": "notifications/initialized"})
    send({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})
    line = proc.stdout.readline()  # wire bytes, trailing newline included
    proc.stdin.close()
    proc.wait(timeout=30)
    tools = json.loads(line)["result"]["tools"]
    return len(line), len(tools), version


def main():
    subprocess.run(["cargo", "build", "-q"], cwd=REPO, check=True)

    home = tempfile.mkdtemp(prefix="foac-context-tax-")
    env = {"HOME": home, "XDG_CONFIG_HOME": f"{home}/.config",
           "LINEAR_API_KEY": "dummy"}

    print(foac(env, "version").decode().strip())

    providers = sorted(json.loads(foac(env, "provider", "list", "--format", "json")))

    print("\n== always-on: skill listing line (frontmatter name + description, UTF-8 bytes) ==")
    listing_total = 0
    skills = {}
    for p in providers:
        skills[p] = foac(env, "skill", "print", p)
        n = len(frontmatter_value(skills[p], "name").encode())
        d = len(frontmatter_value(skills[p], "description").encode())
        listing_total += n + d
        print(f"  foac-{p}: {n + d} B")
    print(f"  one provider active (linear), the bench shape: see foac-linear above")
    print(f"  all {len(providers)} providers active: {listing_total} B")

    print("\n== progressive discovery: --help chain (stdout bytes, one turn each) ==")
    for cmd in (["--help"], ["linear", "--help"],
                ["linear", "issue", "--help"], ["linear", "issue", "list", "--help"]):
        print(f"  foac {' '.join(cmd)}: {len(foac(env, *cmd))} B")

    print("\n== skill route: full provider skill (stdout bytes) ==")
    for p in providers:
        print(f"  foac skill print {p}: {len(skills[p])} B")
    print(f"  all {len(providers)} skills: {sum(len(s) for s in skills.values())} B")

    skipped = False

    print("\n== gh CLI baseline: --help chain (stdout bytes, one turn each) ==")
    gh = shutil.which("gh")
    if gh is None:
        print("  skipped (gh unavailable)")
        skipped = True
    else:
        version = subprocess.run([gh, "--version"], env=env, capture_output=True,
                                 check=True).stdout.decode().splitlines()[0]
        print(f"  {version}")
        for cmd in (["--help"], ["issue", "--help"], ["issue", "list", "--help"]):
            out = subprocess.run([gh, *cmd], env=env, capture_output=True, check=True)
            print(f"  gh {' '.join(cmd)}: {len(out.stdout)} B")

    print(f"\n== MCP baseline: {MCP_IMAGE} tools/list wire bytes ==")
    try:
        wire, tools, version = measure_mcp()
        print(f"  server version {version}: {tools} tools, {wire} B")
    except (OSError, subprocess.SubprocessError, json.JSONDecodeError, KeyError):
        print("  skipped (docker unavailable)")
        skipped = True

    if skipped:
        sys.exit(1)


if __name__ == "__main__":
    main()
