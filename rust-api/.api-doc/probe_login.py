# -*- coding: utf-8 -*-
"""带登录态采集 needLogin 接口真实响应。
登录态：token / userid 作为 query 参数（token_of/userid_of 优先 query），
       dfid 作为 Cookie（dfid_of 优先 cookie），并双保险也放进 query。
写操作（关注/取关、新建/删除歌单、增/删曲目）做配对回滚，避免污染用户账号。
"""
import json, os, time, urllib.parse, urllib.request, ssl

BASE = "http://127.0.0.1:3222"
ROOT = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(ROOT, "responses")
ctx = ssl.create_default_context(); ctx.check_hostname = False; ctx.verify_mode = ssl.CERT_NONE

TOKEN = "3f26d402d64d4276dc8ad8cdd50814fde4440569e1f05c8225d962cb218067ce"
USERID = "487837317"
DFID = "08Tpvm4UNg0g234oDH3kzjk4"

AUTH_Q = {"token": TOKEN, "userid": USERID, "dfid": DFID}
AUTH_COOKIE = "token={}; userid={}; dfid={}".format(TOKEN, USERID, DFID)

HASH = "4F18553F2AF06E355AD75A5EE6173456"      # 一路生花
AAID = "317568292"
MIXSONG = "666075191"
SINGER = "3520"
GCID = "collection_1_509005933_2927982_0"

with open(os.path.join(ROOT, "params.json"), encoding="utf-8") as f:
    meta = {r["route"]: r for r in json.load(f)["routes"]}
with open(os.path.join(ROOT, "manifest.json"), encoding="utf-8") as f:
    manifest = {m["route"]: m for m in json.load(f)}

def call(route, sample, method="GET"):
    """带登录态调用，返回 (status, data_or_None, raw_str, err)。"""
    q = dict(AUTH_Q)
    q.update({k: v for k, v in (sample or {}).items() if v != ""})
    qs = urllib.parse.urlencode(q)
    url = BASE + route + (("?" + qs) if qs else "")
    headers = {
        "User-Agent": "api-doc-probe",
        "Cookie": AUTH_COOKIE,
        "x-apicache-bypass": "1",
    }
    for i in range(2):
        try:
            req = urllib.request.Request(url, headers=headers, method=method)
            with urllib.request.urlopen(req, timeout=25, context=ctx) as resp:
                raw = resp.read().decode("utf-8", "replace")
                st = resp.status
        except urllib.error.HTTPError as e:
            raw = e.read().decode("utf-8", "replace"); st = e.code
        except Exception as e:
            if i == 0:
                time.sleep(1.5); continue
            return None, None, None, str(e)[:160]
        try:
            data = json.loads(raw)
        except Exception:
            return st, None, raw, "non-json"
        return st, data, raw, None
    return None, None, None, "unknown"

def biz_of(d):
    if isinstance(d, dict):
        return d.get("status", d.get("error_code", d.get("errcode", "?")))
    return "?"

def save(route, data, ok, extra=None):
    fname = route.strip("/").replace("/", "__") + ".json"
    out = data if isinstance(data, dict) else {"data": data}
    json.dump(out, open(os.path.join(OUT, fname), "w", encoding="utf-8"),
              ensure_ascii=False, indent=2)
    biz = biz_of(data)
    entry = {"route": route, "url": BASE + route, "login": True,
             "http_status": None if data is None else 200,
             "ok": ok, "biz_status": biz, "file": fname}
    if extra: entry.update(extra)
    manifest[route] = {**manifest.get(route, {}), **entry}
    return fname, biz

# 业务参数（仅 needLogin 与 LOGIN_EXTRA 读类）
SAMPLES = {
    "/artist/follow": {"id": SINGER},
    "/artist/unfollow": {"id": SINGER},
    "/favorite/count": {"mixsongids": MIXSONG},
    "/lastest/songs/listen": {},
    "/login/device": {},
    "/team/period/info": {},
    "/team/my/status": {},
    "/team/my/info": {},
    "/team/my": {},
    "/team/join": {"team_code": "MoeKoeDocTestCode000"},
    "/youth/channel/sub": {"global_collection_id": GCID, "t": "0"},
    "/youth/day/vip": {},
    "/youth/day/vip/upgrade": {},
    "/youth/dynamic": {},
    "/youth/dynamic/recent": {},
    "/youth/listen/song": {"mixsongid": MIXSONG},
    "/youth/month/vip/record": {},
    "/youth/union/vip": {},
    "/youth/vip": {},
    "/personal/fm": {},
    "/playhistory/upload": {"mxid": MIXSONG, "op": "1"},
    "/user/cloud": {"page": "1", "pagesize": "30"},
    "/user/detail": {},
    "/user/follow": {"id": USERID},
    "/user/follow/message": {"id": "1"},
    "/user/history": {"page": "1", "pagesize": "30"},
    "/user/listen": {},
    "/user/playlist": {},
    "/user/video/collect": {"page": "1", "pagesize": "30"},
    "/user/video/love": {"pagesize": "30"},
    # LOGIN_EXTRA 读类
    "/user/vip/detail": {},
    "/playlist/track/all/new": {"listid": GCID},
    "/youth/channel/all": {},
    "/artist/follow/newsongs": {"id": SINGER},
}

results = []
def record(route, data, ok, note=""):
    if data is not None:
        save(route, data, ok)
        results.append((route, "OK" if ok else "BIZ-ERR", note))
    else:
        results.append((route, "FAIL", note))

# —— 普通路由逐个调用 ——
for route, sample in SAMPLES.items():
    if route in ("/artist/follow", "/artist/unfollow"):
        continue  # 单独处理回滚链
    if route in ("/playlist/add", "/playlist/del", "/playlist/tracks/add", "/playlist/tracks/del"):
        continue  # 单独处理回滚链
    st, data, raw, err = call(route, sample)
    if data is not None:
        biz = biz_of(data)
        ok = (biz == 1) or (isinstance(biz, int) and biz == 0 and isinstance(data, dict) and "status" not in data)
        record(route, data, ok, "biz=%s" % biz)
    else:
        results.append((route, "FAIL", err or ""))
    time.sleep(0.6)

# —— artist follow / unfollow 回滚 ——
st, fdata, _, _ = call("/artist/follow", {"id": SINGER})
if fdata is not None:
    save("/artist/follow", fdata, True)
    results.append(("/artist/follow", "OK", "follow"))
st, udata, _, _ = call("/artist/unfollow", {"id": SINGER})
if udata is not None:
    save("/artist/unfollow", udata, True)
    results.append(("/artist/unfollow", "OK", "unfollow(rollback)"))
time.sleep(0.5)

# —— playlist add -> tracks add -> tracks del -> playlist del 回滚链 ——
st, add_data, _, e1 = call("/playlist/add", {"name": "MoeKoeDocTest", "type": "0", "source": "1"})
if add_data is not None:
    save("/playlist/add", add_data, True)
    results.append(("/playlist/add", "OK", "create test playlist"))
    # 解析 listid / gcid
    d = add_data.get("data", add_data)
    if isinstance(d, dict):
        listid = d.get("listid") or d.get("specialid") or (d.get("data") or {}).get("listid")
        gcid = d.get("global_collection_id") or (d.get("data") or {}).get("global_collection_id")
    else:
        listid = gcid = None
    # 往测试歌单加一首歌
    st2, ta_data, _, e2 = call("/playlist/tracks/add",
                                {"listid": gcid or str(listid),
                                 "data": "一路生花|" + HASH})
    if ta_data is not None:
        save("/playlist/tracks/add", ta_data, True)
        results.append(("/playlist/tracks/add", "OK", "add track to test playlist"))
        # 取刚加曲目的 fileid
        _, ta_list, _, _ = call("/playlist/track/all", {"id": gcid or str(listid), "pagesize": "10"})
        fileid = None
        if ta_list and isinstance(ta_list.get("data"), dict):
            for key in ("list", "songs", "info"):
                arr = ta_list["data"].get(key)
                if isinstance(arr, list) and arr:
                    fileid = arr[0].get("fileid") or arr[0].get("FileID")
                    break
        if fileid:
            st3, td_data, _, e3 = call("/playlist/tracks/del",
                                       {"listid": gcid or str(listid), "fileids": str(fileid)})
            if td_data is not None:
                save("/playlist/tracks/del", td_data, True)
                results.append(("/playlist/tracks/del", "OK", "del track (rollback)"))
            else:
                results.append(("/playlist/tracks/del", "FAIL", e3 or ""))
        else:
            results.append(("/playlist/tracks/del", "SKIP", "no fileid parsed"))
    else:
        results.append(("/playlist/tracks/add", "FAIL", e2 or ""))
    # 删除测试歌单（回滚）
    if listid:
        st4, del_data, _, e4 = call("/playlist/del", {"listid": str(listid)})
        if del_data is not None:
            save("/playlist/del", del_data, True)
            results.append(("/playlist/del", "OK", "delete test playlist (rollback)"))
        else:
            results.append(("/playlist/del", "FAIL", e4 or ""))
else:
    results.append(("/playlist/add", "FAIL", e1 or ""))

# 合并 manifest
json.dump(list(manifest.values()), open(os.path.join(ROOT, "manifest.json"), "w",
          encoding="utf-8"), ensure_ascii=False, indent=2)
print("=== 采集完成 ===")
for r in results:
    print("  %-32s %-9s %s" % (r[0], r[1], r[2]))
print("共 %d 项" % len(results))
