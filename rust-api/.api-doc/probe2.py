# -*- coding: utf-8 -*-
"""第二轮补采：修正示例参数后重试失败的公开接口。"""
import json, os, time, urllib.parse, urllib.request, ssl

BASE = "http://127.0.0.1:3222"
ROOT = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(ROOT, "responses")
ctx = ssl.create_default_context(); ctx.check_hostname = False; ctx.verify_mode = ssl.CERT_NONE

HASH = "4F18553F2AF06E355AD75A5EE6173456"      # 一路生花
AAID = "317568292"                              # 一路生花 album_audio_id
GCID = "collection_1_509005933_2927982_0"       # 真实歌单 gcid
SINGER = "3520"

# route -> 覆盖用的 sample（仅非登录接口）
RETRY = {
    "/search": {"keywords": "一路生花", "pagesize": 5},
    "/search/mixed": {"keywords": "一路生花"},
    "/search/complex": {"keywords": "一路生花"},
    "/lyric": {"id": HASH},
    "/song/url/new": {"hash": HASH, "album_audio_id": AAID},
    "/audio/related": {"hash": HASH, "album_audio_id": AAID},
    "/kmr/audio/mv": {"hash": HASH},
    "/artist/audios": {"id": SINGER},
    "/artist/videos": {"id": SINGER},
    "/artist/follow/newsongs": {"id": SINGER},
    "/comment/music": {"mixsongid": AAID},
    "/comment/music/classify": {"mixsongid": AAID},
    "/comment/floor": {"mixsongid": AAID},
    "/song/ranking": {"hash": HASH},
    "/song/ranking/filter": {"hash": HASH},
    "/sheet/list": {},
    "/scene/audio/list": {},
    "/yueku/banner": {},
    "/top/card/youth": {},
    "/ip/zone/home": {},
    "/youth/channel/all": {},
    "/playlist/track/all": {"id": GCID, "pagesize": 5},
    "/playlist/track/all/new": {"listid": GCID},
    "/playlist/detail": {"ids": GCID},
    "/playlist/similar": {"ids": GCID},
    "/video/url": {"hash": HASH},
    "/video/detail": {"hash": HASH},
    "/krm/audio": {"hash": HASH},
    "/song/url": {"hash": HASH},
}

with open(os.path.join(ROOT, "params.json"), encoding="utf-8") as f:
    meta = {r["route"]: r for r in json.load(f)["routes"]}
with open(os.path.join(ROOT, "manifest.json"), encoding="utf-8") as f:
    manifest = {m["route"]: m for m in json.load(f)}

def call(route, sample, attempts=2):
    qs = urllib.parse.urlencode({k: v for k, v in sample.items() if v != ""})
    url = BASE + route + (("?" + qs) if qs else "")
    for i in range(attempts):
        try:
            req = urllib.request.Request(url, headers={"User-Agent": "api-doc-probe"})
            with urllib.request.urlopen(req, timeout=20, context=ctx) as resp:
                return resp.status, resp.read().decode("utf-8", "replace"), None
        except urllib.error.HTTPError as e:
            body = e.read().decode("utf-8", "replace")
            # 502 错误体通常仍是 JSON 信封
            try:
                json.loads(body)
                return e.code, body, None
            except Exception:
                err = f"HTTP {e.code}"
        except Exception as e:
            err = str(e)[:150]
        if i < attempts - 1:
            time.sleep(1.5)
    return None, None, err

results = []
for route, override in RETRY.items():
    need_login = meta.get(route, {}).get("needLogin", False)
    if need_login:
        print("SKIP(login) " + route, flush=True)
        continue
    sample = dict(meta.get(route, {}).get("sample") or {})
    sample.update(override)
    st, body, err = call(route, sample)
    fname = route.strip("/").replace("/", "__") + ".json"
    entry = {"route": route, "url": BASE + route, "retry": True, "http_status": st,
             "ok": False, "error": err}
    if body is not None:
        try:
            data = json.loads(body)
            json.dump(data, open(os.path.join(OUT, fname), "w", encoding="utf-8"),
                      ensure_ascii=False, indent=2)
            entry["ok"] = True
            entry["biz_status"] = data.get("status", data.get("error_code",
                                     data.get("errcode", "?")))
        except Exception:
            entry["error"] = "non-json"
    results.append(entry)
    print(("OK  " if entry["ok"] else "FAIL") + " " + route +
          (" " + str(entry.get("biz_status", "")) if entry["ok"] else
           " " + (entry["error"] or "")), flush=True)
    time.sleep(0.8)

# 合并进 manifest
for e in results:
    manifest[e["route"]] = {**manifest.get(e["route"], {}), **e}
json.dump(list(manifest.values()), open(os.path.join(ROOT, "manifest.json"), "w",
          encoding="utf-8"), ensure_ascii=False, indent=2)
print("DONE", len(results))
