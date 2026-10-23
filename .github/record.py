#!/usr/bin/env python3
"""Record every conformance case of the contract the adapter speaks.

**This is CI harness, not plugin content.** Nothing under `.github/` is
installed, and lemonfiber never runs any of it (`F3-R6`).

Each recording under `conformance/<capability>@<major>/<case>.json` is the
adapter's answer to one case of that contract's `conformance.json` (ARCH-R217),
asked with the case's own `asked`, in the order the contract lists them. It is
taken against a throwaway qBittorrent at the digest `plugin.toml` pins for the
plugin's own service, on a network with no route off the host, so nothing it
answers depends on a tracker or a peer. The adapter runs from this repository's
Dockerfile, as lemonfiber runs it: the caller's uid, read-only, no
capabilities, the key and the upstream credential owner-only in its
configuration directory. A one-shot container in qBittorrent's own network
namespace stands where lemonfiber stands: handed the temporary password qBittorrent announces in
its log on its first start, it mints the run's password inside the run, sets it
in qBittorrent, proves it, and writes it to the adapter's `upstream.json` beside
`lemonfiber.key`. This script never mints or writes qBittorrent's password; it
reads `upstream.json` back only to refuse any answer that holds it.

    python3 .github/record.py            write conformance/
    python3 .github/record.py --check    record again and compare

Needs Docker. Exit 0 = recorded (or matched), 1 = it did not.
"""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import re
import secrets
import subprocess
import sys
import tempfile
import time
import tomllib
import urllib.error
import urllib.parse
import urllib.request
import uuid

ROOT = pathlib.Path(__file__).resolve().parents[1]
RECORDINGS = ROOT / "conformance"
CONTRACTS = "https://raw.githubusercontent.com/lemonfiber/lemonfiber/{rev}/contract/capabilities/{capability}/v{major}/conformance.json"
JSON = "application/json"
STARTUP_S = 180
ANSWER_S = 30
ADAPTER_PORT = 8080
SEGMENT = re.compile(r"[a-z][a-z0-9_.]*")
MAJOR = re.compile(r"[1-9]\d*")
USERNAME = "admin"
TEMPORARY = "A temporary password is provided for this session:"
SEED = r"""
set -eu
read -r temporary
base="http://localhost:$LISTENS/api/v2"
jar=$(mktemp)
ask() { curl -sS --retry 30 --retry-connrefused --retry-delay 2 -c "$jar" -b "$jar" "$@"; }
minted=$(tr -dc 'A-Za-z0-9' </dev/urandom | head -c 32)
ask --data-urlencode "username=$USERNAME" --data-urlencode "password=$temporary" "$base/auth/login" >/dev/null
ask --data-urlencode "json={\"web_ui_password\":\"$minted\"}" "$base/app/setPreferences" >/dev/null
ask -X POST "$base/auth/logout" >/dev/null
ask --data-urlencode "username=$USERNAME" --data-urlencode "password=$minted" "$base/auth/login" >/dev/null
test "$(ask -o /dev/null -w '%{http_code}' "$base/app/version")" = 200
rm -f "$jar"
umask 077
printf '{"username":"%s","password":"%s"}' "$USERNAME" "$minted" > /adapter/upstream.json
chown "$OWNER" /adapter/upstream.json
"""
NOTES = {
    "refuses-without-the-key": "Asked without the plugin's key: the kit refuses it before the adapter is reached.",
    "transfers-answers": "What a fresh qBittorrent is downloading: nothing.",
    "pulling-answers": "Whether a fresh qBittorrent is fetching: nothing runs, and it would start the next torrent.",
    "stop-answers": "New torrents added stopped and every torrent stopped, read back as stopped.",
    "resume-answers": "New torrents added started and every torrent started, read back as fetching.",
    "throttled-answers": "The rates a fresh qBittorrent is held to: none, and no schedule.",
    "restrain-answers": "Held to the active rates around the clock with no window, and read back.",
    "moving-answers": "How fast it is moving with nothing to move.",
    "moved-answers": "What it has moved since it started, which is all it keeps, and says so.",
    "seeding-answers": "What a fresh qBittorrent is seeding: nothing.",
    "stop_seeding-answers": "A torrent it does not hold, refused as the contract declares.",
    "filed-answers": "Every category a fresh qBittorrent holds: none.",
    "file_under-answers": "A category filed under the folder the case names, with torrents managed automatically.",
    "peer_port-answers": "The port a fresh qBittorrent takes peers on.",
    "set_peer_port-answers": "The port it takes peers on set, and read back.",
}


def docker(*args: str, check: bool = True) -> str:
    done = subprocess.run(["docker", *args], capture_output=True, text=True, check=False)
    if check and done.returncode != 0:
        raise RuntimeError(f"docker {' '.join(args)}: {done.stderr.strip()}")
    return done.stdout.strip()


def manifest() -> tuple[str, str, int]:
    """The plugin's own service: its id, its pinned image and the port it listens on."""
    read = tomllib.loads((ROOT / "plugin.toml").read_text(encoding="utf-8"))
    plugin = read["plugin"]["id"]
    upstream = next(one for one in read["service"] if one["id"] == plugin)
    return plugin, f"{upstream['image']}@{upstream['digest']}", upstream["listens"]


def core_rev() -> str:
    read = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    return read["dependencies"]["lemonfiber-adapter-kit"]["rev"]


def published_cases(spoken: str) -> list[dict]:
    capability, major = spoken.split("@")
    url = CONTRACTS.format(rev=core_rev(), capability=capability, major=major)
    with urllib.request.urlopen(url, timeout=ANSWER_S) as answer:
        return json.load(answer)["cases"]


def asked(base: str, path: str, key: str | None, body: dict | None = None) -> tuple[int, str, str]:
    request = urllib.request.Request(
        f"{base}{path}",
        data=None if body is None else json.dumps(body).encode(),
        method="GET" if body is None else "POST",
        headers={"Content-Type": JSON, "Accept": JSON} | ({"Authorization": f"Bearer {key}"} if key else {}),
    )
    try:
        with urllib.request.urlopen(request, timeout=ANSWER_S) as answer:
            return answer.status, answer.headers.get("content-type", ""), answer.read().decode()
    except urllib.error.HTTPError as refused:
        return refused.code, refused.headers.get("content-type", ""), refused.read().decode()


def recording(pinned: str, path: str, status: int, kind: str, body: str, note: str) -> dict:
    response: dict = {"status": status}
    if kind:
        response["headers"] = {"content-type": kind}
    if body.strip() == "null":
        response["body_starts_with"] = "null"
    elif body.strip():
        response["json"] = json.loads(body)
    return {
        "recorded_from": pinned,
        "note": note,
        "request": {"method": "POST", "path": path, "accept": JSON},
        "response": response,
    }


def temporary_password(log: str) -> str | None:
    """The temporary password qBittorrent announced last in its log, as lemonfiber reads it."""
    for line in reversed(log.splitlines()):
        _, found, password = line.partition(TEMPORARY)
        if found and password.strip():
            return password.strip()
    return None


def seeded(name: str, upstream: str, listens: int, pinned: str, adapter: pathlib.Path,
           temporary: str) -> None:
    """The run's password minted and set inside a one-shot container, and written to `upstream.json`."""
    done = subprocess.run(
        ["docker", "run", "--rm", "-i", "--name", f"{name}-seed", "--network", f"container:{upstream}",
         "--entrypoint", "sh", "-e", f"LISTENS={listens}", "-e", f"USERNAME={USERNAME}",
         "-e", f"OWNER={os.getuid()}:{os.getgid()}", "-v", f"{adapter}:/adapter", pinned, "-c", SEED],
        input=f"{temporary}\n", capture_output=True, text=True, check=False)
    if done.returncode != 0:
        raise RuntimeError(f"qBittorrent did not take the password minted for the run: {done.stderr.strip()}")


def owner_only(directory: pathlib.Path) -> None:
    for file in directory.iterdir():
        file.chmod(0o600)
    directory.chmod(0o700)


def waited(what: str, ready) -> object:
    deadline = time.monotonic() + STARTUP_S
    while time.monotonic() < deadline:
        found = ready()
        if found:
            return found
        time.sleep(2)
    raise RuntimeError(f"{what} within {STARTUP_S}s")


def recorded_live(plugin: str, pinned: str, listens: int) -> tuple[list[str], dict[tuple[str, str], dict]]:
    """What the adapter speaks, and every case asked of it in front of a throwaway qBittorrent."""
    name = f"{plugin}-record-{uuid.uuid4().hex[:8]}"
    inside, outside = f"{name}-inside", f"{name}-outside"
    key = secrets.token_hex(24)
    with tempfile.TemporaryDirectory() as held:
        held = pathlib.Path(held)
        upstream, data, adapter = held / "upstream", held / "data", held / "adapter"
        for directory in (upstream, data, adapter):
            directory.mkdir()
        try:
            docker("network", "create", "--internal", inside)
            docker("network", "create", outside)
            docker("run", "-d", "--name", f"{name}-upstream", "--network", inside,
                   "--network-alias", plugin, "-e", f"PUID={os.getuid()}", "-e", f"PGID={os.getgid()}",
                   "-e", "TZ=Etc/UTC", "-e", f"WEBUI_PORT={listens}",
                   "-v", f"{upstream}:/config", "-v", f"{data}:/data", pinned)
            temporary = waited("qBittorrent announced no temporary password",
                               lambda: temporary_password(docker("logs", f"{name}-upstream", check=False)))
            (adapter / "lemonfiber.key").write_text(key, encoding="utf-8")
            seeded(name, f"{name}-upstream", listens, pinned, adapter, temporary)
            owner_only(adapter)
            docker("build", "-q", "-t", f"{name}:adapter", str(ROOT))
            docker("create", "--name", f"{name}-adapter", "--network", outside,
                   "--user", f"{os.getuid()}:{os.getgid()}", "--read-only", "--cap-drop", "ALL",
                   "--security-opt", "no-new-privileges", "-v", f"{adapter}:/config:ro",
                   "-p", f"127.0.0.1::{ADAPTER_PORT}", f"{name}:adapter")
            docker("network", "connect", inside, f"{name}-adapter")
            docker("start", f"{name}-adapter")
            port = docker("port", f"{name}-adapter", str(ADAPTER_PORT)).splitlines()[0].rsplit(":", 1)[1]
            base = f"http://127.0.0.1:{port}"
            waited("the adapter was not ready", lambda: answering(base, key))
            speaks = json.loads(asked(base, "/lemonfiber/adapter/v1/about", key)[2])["speaks"]
            said = recorded(base, key, pinned, speaks)
            minted = json.loads((adapter / "upstream.json").read_text(encoding="utf-8"))["password"]
            leaked = [case for case, one in said.items()
                      if any(secret in json.dumps(one) for secret in (minted, temporary, key))]
            if leaked:
                raise RuntimeError(f"an answer holds a credential, so nothing is written: {leaked}")
            return speaks, said
        finally:
            for container in (f"{name}-adapter", f"{name}-seed", f"{name}-upstream"):
                docker("rm", "-fv", container, check=False)
            docker("rmi", f"{name}:adapter", check=False)
            for network in (inside, outside):
                docker("network", "rm", network, check=False)
            docker("run", "--rm", "-v", f"{held}:/held", "--entrypoint", "chown", pinned,
                   "-R", f"{os.getuid()}:{os.getgid()}", "/held", check=False)


def answering(base: str, key: str) -> bool:
    try:
        status, _, body = asked(base, "/lemonfiber/adapter/v1/ready", key)
    except (urllib.error.URLError, ConnectionError):
        return False
    return status == 200 and body.strip() == "true"


def recorded(base: str, key: str, pinned: str, speaks: list[str]) -> dict[tuple[str, str], dict]:
    """Every case of every contract spoken, asked in the order each contract lists them."""
    said: dict[tuple[str, str], dict] = {}
    for spoken in speaks:
        capability, major = spoken.split("@")
        for case in published_cases(spoken):
            path = operation_path(capability, major, case["operation"])
            status, kind, body = asked(base, path, key if case["keyed"] else None, case["asked"])
            note = NOTES.get(case["case"], "")
            if not note:
                raise RuntimeError(f"{spoken}/{case['case']} has no note to record it with")
            said[(spoken, case["case"])] = recording(pinned, path, status, kind, body, note)
    return said


def operation_path(capability: str, major: str, operation: str) -> str:
    """Where an operation is asked, each segment held to the shape the contract writes it in."""
    for segment, shape in ((capability, SEGMENT), (major, MAJOR), (operation, SEGMENT)):
        if not shape.fullmatch(segment):
            raise RuntimeError(f"{segment!r} is not a segment a contract names")
    quoted = [urllib.parse.quote(segment, safe="") for segment in (capability, major, operation)]
    return "/lemonfiber/{}/v{}/{}".format(*quoted)


def filed(said: dict[tuple[str, str], dict], spoken: str, case: str, checking: bool) -> str | None:
    """One case written to conformance/, or compared with what is there; the fault, if any."""
    got = said.get((spoken, case))
    path = RECORDINGS / spoken / f"{case}.json"
    if got is None:
        return f"{spoken}/{case} was not asked"
    if checking:
        held = json.loads(path.read_text(encoding="utf-8")) if path.is_file() else None
        return None if held == got else f"{path.relative_to(ROOT)} is not what qBittorrent answers now"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(got, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--check", action="store_true", help="record again and compare with conformance/")
    checking = parser.parse_args().check

    plugin, pinned, listens = manifest()
    speaks, said = recorded_live(plugin, pinned, listens)
    faults = [fault for spoken in speaks for case in published_cases(spoken)
              if (fault := filed(said, spoken, case["case"], checking)) is not None]
    for fault in faults:
        print(f"::error::{fault}")
    print(f"{len(said)} cases asked against {pinned}; {len(faults)} fault(s).")
    return 1 if faults else 0


if __name__ == "__main__":
    sys.exit(main())
