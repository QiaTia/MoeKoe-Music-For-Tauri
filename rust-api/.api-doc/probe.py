# -*- coding: utf-8 -*-
"""探测 rust-api 全部接口，采集真实返回数据。"""
import json, os, time, urllib.parse, urllib.request, ssl

BASE = "http://127.0.0.1:3222"
ROOT = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(ROOT, "responses")
os.makedirs(OUT, exist_ok=True)

ctx = ssl.create_default_context()
ctx.check_hostname = False
ctx.verify_mode = ssl.CERT_NONE

with open(os.path.join(ROOT, "params.json"), encoding="utf-8") as f:
    data = json.load(f)

manifest = []
for r in data["routes"]:
    route = r["route"]
    sample = r.get("sample") or {}
    qs = urllib.parse.urlencode({k: v for k, v in sample.items() if v != ""})
    url = BASE + route + (("?" + qs) if qs else "")
    fname = route.strip("/").replace("/", "__") + ".json"
    entry = {"route": route, "url": url, "file": fname, "http_status": None,
             "ok": False, "error": None, "needLogin": r.get("needLogin", False)}
    try:
        req = urllib.request.Request(url, headers={"User-Agent": "api-doc-probe"})
        t0 = time.time()
        with urllib.request.urlopen(req, timeout=20, context=ctx) as resp:
            body = resp.read().decode("utf-8", "replace")
            entry["http_status"] = resp.status
        entry["ms"] = int((time.time() - t0) * 1000)
        try:
            parsed = json.loads(body)
            with open(os.path.join(OUT, fname), "w", encoding="utf-8") as fo:
                json.dump(parsed, fo, ensure_ascii=False, indent=2)
            entry["ok"] = True
            # 记录业务状态
            entry["biz_status"] = parsed.get("status", parsed.get("error_code",
                                   parsed.get("errcode", "?")))
        except json.JSONDecodeError:
            with open(os.path.join(OUT, fname), "w", encoding="utf-8") as fo:
                fo.write(body)
            entry["ok"] = True
            entry["note"] = "non-json"
    except Exception as e:
        entry["error"] = str(e)[:200]
    manifest.append(entry)
    print(("OK  " if entry["ok"] else "FAIL") + " " + route +
          (" " + str(entry.get("biz_status", "")) if entry["ok"] else
           " " + (entry["error"] or "")), flush=True)
    time.sleep(0.3)

with open(os.path.join(ROOT, "manifest.json"), "w", encoding="utf-8") as f:
    json.dump(manifest, f, ensure_ascii=False, indent=2)
print("DONE", len(manifest))
