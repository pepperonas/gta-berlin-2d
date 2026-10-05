#!/usr/bin/env python3
"""Freesound-Anbindung für tools/audio/build_sfx.py (Suche, Anmeldung, Download der Originaldateien).

Zugangsdaten liegen NIE im Repo, sondern in ~/.config/gta-berlin/freesound.json:
  {"client_id": "…", "client_secret": "…"}            (von https://freesound.org/apiv2/apply)
Nach der Anmeldung kommen access_token/refresh_token dazu (OAuth2, für Originaldateien nötig).

  python tools/audio/freesound.py login              Anmelde-Link ausgeben
  python tools/audio/freesound.py code CODE          angezeigten Code gegen Tokens tauschen
  python tools/audio/freesound.py search "car horn" [--cc0] [--max-dur 5]
                                                     Kandidaten: ID, Name, Lizenz, Dauer, Format, Bewertung
  python tools/audio/freesound.py info ID            Metadaten eines Klangs
"""
import json
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

CONF = Path.home() / ".config/gta-berlin/freesound.json"
API = "https://freesound.org/apiv2"
LICENSES = {
    "http://creativecommons.org/publicdomain/zero/1.0/": "CC0 1.0",
    "https://creativecommons.org/publicdomain/zero/1.0/": "CC0 1.0",
    "http://creativecommons.org/licenses/by/3.0/": "CC BY 3.0",
    "https://creativecommons.org/licenses/by/4.0/": "CC BY 4.0",
    "http://creativecommons.org/licenses/by/4.0/": "CC BY 4.0",
}


def conf() -> dict:
    if not CONF.exists():
        sys.exit(f"Zugangsdaten fehlen: {CONF} mit client_id und client_secret anlegen")
    return json.loads(CONF.read_text())


def save(c: dict) -> None:
    CONF.write_text(json.dumps(c, indent=2))
    CONF.chmod(0o600)


def license_name(url: str) -> str:
    if url not in LICENSES:
        sys.exit(f"Lizenz nicht erlaubt (nur CC0/CC BY): {url}")
    return LICENSES[url]


def _post_token(data: dict) -> None:
    c = conf()
    data = {"client_id": c["client_id"], "client_secret": c["client_secret"], **data}
    req = urllib.request.Request(f"{API}/oauth2/access_token/", data=urllib.parse.urlencode(data).encode())
    r = json.loads(urllib.request.urlopen(req).read())
    c["access_token"] = r["access_token"]
    c["refresh_token"] = r["refresh_token"]
    c["expires_at"] = time.time() + r["expires_in"] - 60
    save(c)


def token() -> str:
    c = conf()
    if "refresh_token" not in c:
        sys.exit("noch nicht angemeldet: python tools/audio/freesound.py login")
    if time.time() > c.get("expires_at", 0):
        _post_token({"grant_type": "refresh_token", "refresh_token": c["refresh_token"]})
        c = conf()
    return c["access_token"]


def get(path: str, params: dict | None = None, oauth: bool = False) -> dict:
    params = dict(params or {})
    headers = {"User-Agent": "gta-berlin-build"}
    if oauth:
        headers["Authorization"] = f"Bearer {token()}"
    else:
        params["token"] = conf()["client_secret"]
    url = f"{API}{path}?{urllib.parse.urlencode(params)}"
    return json.loads(urllib.request.urlopen(urllib.request.Request(url, headers=headers)).read())


def info(sid: int) -> dict:
    return get(f"/sounds/{sid}/", {"fields": "id,name,username,license,duration,type,samplerate,channels,url,avg_rating,num_downloads"})


def download(sid: int) -> bytes:
    """Originaldatei; bei Drosselung (HTTP 429) mit wachsender Pause erneut versuchen."""
    for attempt in range(8):
        req = urllib.request.Request(
            f"{API}/sounds/{sid}/download/",
            headers={"Authorization": f"Bearer {token()}", "User-Agent": "gta-berlin-build"},
        )
        try:
            return urllib.request.urlopen(req).read()
        except urllib.error.HTTPError as e:
            if e.code != 429:
                raise
            wait = float(e.headers.get("Retry-After") or 0) or 10 * 2**attempt
            print(f"Freesound drosselt, warte {wait:.0f} s", file=sys.stderr)
            time.sleep(min(wait, 600))
    sys.exit(f"Download von {sid} scheitert dauerhaft (429)")


def search(q: str, cc0: bool, max_dur: float | None) -> None:
    flt = ['license:("Creative Commons 0" OR "Attribution")']
    if cc0:
        flt = ['license:"Creative Commons 0"']
    if max_dur:
        flt.append(f"duration:[0 TO {max_dur}]")
    r = get(
        "/search/text/",
        {
            "query": q,
            "filter": " ".join(flt),
            "sort": "rating_desc",
            "page_size": 30,
            "fields": "id,name,username,license,duration,type,samplerate,avg_rating,num_downloads",
        },
    )
    for s in r["results"]:
        lic = LICENSES.get(s["license"], s["license"])
        print(
            f"{s['id']:>7}  {s['duration']:6.1f}s  {s['type']:4} {int(s['samplerate']):>6}  "
            f"★{s['avg_rating']:.1f} ↓{s['num_downloads']:<6} {lic:9}  {s['username']}: {s['name']}"
        )


if __name__ == "__main__":
    a = sys.argv[1:]
    if not a:
        sys.exit(__doc__)
    if a[0] == "login":
        c = conf()
        print(f"{API}/oauth2/authorize/?client_id={c['client_id']}&response_type=code")
    elif a[0] == "code":
        _post_token({"grant_type": "authorization_code", "code": a[1]})
        print("angemeldet, Tokens gespeichert in", CONF)
    elif a[0] == "search":
        md = float(a[a.index("--max-dur") + 1]) if "--max-dur" in a else None
        search(a[1], "--cc0" in a, md)
    elif a[0] == "info":
        print(json.dumps(info(int(a[1])), indent=2, ensure_ascii=False))
