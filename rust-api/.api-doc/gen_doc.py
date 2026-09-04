# -*- coding: utf-8 -*-
"""根据 params.json + responses/ + manifest.json 生成接口文档 Markdown。"""
import json, os

ROOT = os.path.dirname(os.path.abspath(__file__))
DOCS_OUT = r"D:\Project\MoeKoeMusic\rust-api\docs\API.md"

# 路由 → 中文说明
DESC = {
    "/search": "歌曲搜索",
    "/search/complex": "综合搜索（多类型聚合结果）",
    "/search/default": "搜索默认词（打开搜索页时的推荐词）",
    "/search/hot": "热门搜索词",
    "/search/mixed": "混合搜索（专辑/歌手/歌单/用户等聚合）",
    "/search/lyric": "歌词搜索（按歌词内容找歌）",
    "/search/suggest": "搜索建议（输入联想）",
    "/lyric": "获取歌词（含 KRC 解密，翻译/罗马音）",
    "/song/url": "获取歌曲播放地址",
    "/song/url/new": "获取歌曲播放地址（新版）",
    "/song/climax": "获取歌曲高潮点（副歌时间段）",
    "/song/ranking": "歌曲在排行榜中的排名",
    "/song/ranking/filter": "歌曲上榜筛选信息",
    "/album": "专辑信息",
    "/album/detail": "专辑详情",
    "/album/songs": "专辑内歌曲列表",
    "/album/shop": "专辑商城信息（售卖/价格）",
    "/artist/albums": "歌手专辑列表",
    "/artist/audios": "歌手音频列表",
    "/artist/detail": "歌手详情",
    "/artist/follow": "关注歌手",
    "/artist/follow/newsongs": "关注歌手的新歌",
    "/artist/honour": "歌手荣誉/奖项",
    "/artist/lists": "歌手分类列表",
    "/artist/unfollow": "取消关注歌手",
    "/artist/videos": "歌手视频列表",
    "/audio": "音频（歌曲）详情",
    "/audio/accompany/matching": "伴奏匹配",
    "/audio/ktv/total": "KTV 演唱次数统计",
    "/audio/related": "相关音频推荐",
    "/brush": "刷一刷（随机推荐）",
    "/captcha/sent": "发送短信验证码",
    "/comment/album": "专辑评论",
    "/comment/count": "评论数量",
    "/comment/floor": "评论楼层（回复楼中楼）",
    "/comment/music": "歌曲评论",
    "/comment/music/classify": "歌曲评论分类（热评类型）",
    "/comment/music/hotword": "评论热词",
    "/comment/playlist": "歌单评论",
    "/everyday/friend": "每日好友（好友在听）",
    "/everyday/history": "每日推荐历史",
    "/everyday/recommend": "每日歌曲推荐",
    "/everyday/style/recommend": "每日风格推荐",
    "/favorite/count": "收藏数量统计",
    "/fm/class": "电台分类",
    "/fm/image": "电台图片",
    "/fm/recommend": "电台推荐",
    "/fm/songs": "电台歌曲列表",
    "/ip": "IP 信息",
    "/ip/dateil": "IP 归属地详情",
    "/ip/playlist": "地域歌单",
    "/ip/zone": "IP 地域信息",
    "/ip/zone/home": "地域主页",
    "/kmr/audio/mv": "KMR 音频关联 MV",
    "/krm/audio": "KRM 音频信息",
    "/lastest/songs/listen": "最近在听歌曲",
    "/longaudio/album/audios": "长音频专辑内音频列表",
    "/longaudio/album/detail": "长音频专辑详情",
    "/longaudio/daily/recommend": "长音频每日推荐",
    "/longaudio/rank/recommend": "长音频榜单推荐",
    "/longaudio/vip/recommend": "长音频 VIP 推荐",
    "/longaudio/week/recommend": "长音频每周推荐",
    "/login": "登录（账号相关聚合入口）",
    "/login/cellphone": "手机号登录",
    "/login/token": "token 登录（换票）",
    "/login/qr/key": "二维码登录 key",
    "/login/qr/create": "二维码登录生成二维码",
    "/login/qr/check": "二维码登录状态检测",
    "/login/device": "登录设备列表",
    "/login/device/kick": "踢下线设备",
    "/login/openplat": "第三方开放平台登录",
    "/login/wx/check": "微信登录状态检测",
    "/login/wx/create": "微信登录二维码",
    "/images": "图片素材",
    "/images/audio": "音频图片（封面等）",
    "/recommend/songs": "推荐歌曲",
    "/rank/audio": "榜内音频",
    "/rank/info": "榜单信息",
    "/rank/list": "排行榜列表",
    "/rank/top": "榜单置顶/头部内容",
    "/rank/vol": "榜单期数（卷）",
    "/singer/list": "歌手列表（按分类）",
    "/sheet/collection": "歌单合集",
    "/sheet/collection/detail": "歌单合集详情",
    "/sheet/detail": "歌单详情",
    "/sheet/hot": "热门歌单",
    "/sheet/list": "歌单列表",
    "/top/album": "专辑榜",
    "/top/card": "顶部卡片（首页运营位）",
    "/top/card/youth": "青年模式顶部卡片",
    "/top/ip": "地域热榜",
    "/top/playlist": "歌单榜",
    "/top/song": "新歌榜/顶部分类歌曲",
    "/scene/audio/list": "场景下音频列表",
    "/scene/collection/list": "场景合集列表",
    "/scene/lists": "场景列表",
    "/scene/lists/v2": "场景列表（v2）",
    "/scene/module": "场景模块",
    "/scene/module/info": "场景模块信息",
    "/scene/music": "场景音乐",
    "/scene/video/list": "场景视频列表",
    "/yueku": "乐库内容",
    "/yueku/banner": "乐库 Banner",
    "/yueku/fm": "乐库电台",
    "/theme/music": "主题音乐",
    "/theme/music/detail": "主题音乐详情",
    "/theme/playlist": "主题歌单",
    "/theme/playlist/track": "主题歌单内曲目",
    "/team/period/info": "组队活动期次信息",
    "/team/my/status": "我的组队状态",
    "/team/my/info": "我的组队信息",
    "/team/my": "我的组队活动",
    "/team/join": "加入组队",
    "/youth/channel/all": "青年频道列表",
    "/youth/channel/amway": "青年频道安利内容",
    "/youth/channel/detail": "青年频道详情",
    "/youth/channel/similar": "相似频道推荐",
    "/youth/channel/song": "频道内歌曲",
    "/youth/channel/song/detail": "频道歌曲详情",
    "/youth/channel/sub": "订阅/退订频道",
    "/youth/day/vip": "青春每日 VIP 信息",
    "/youth/day/vip/upgrade": "青春每日 VIP 升级",
    "/youth/dynamic": "青年动态",
    "/youth/dynamic/recent": "青年最近动态",
    "/youth/listen/song": "青年模式在听歌曲",
    "/youth/month/vip/record": "青春月 VIP 记录",
    "/youth/union/vip": "青春联合 VIP",
    "/youth/user/song": "青年用户歌曲",
    "/youth/vip": "青春 VIP 信息",
    "/playlist/add": "新建歌单",
    "/playlist/del": "删除歌单",
    "/playlist/detail": "歌单详情",
    "/playlist/effect": "歌单效果（标签氛围）",
    "/playlist/similar": "相似歌单推荐",
    "/playlist/tags": "歌单标签",
    "/playlist/track/all": "歌单内全部曲目",
    "/playlist/track/all/new": "歌单内全部曲目（新版）",
    "/playlist/tracks/add": "歌单添加歌曲",
    "/playlist/tracks/del": "歌单移除歌曲",
    "/pc/diantai": "PC 电台",
    "/personal/fm": "私人 FM",
    "/playhistory/upload": "上报播放历史",
    "/privilege/lite": "歌曲权限（精简版）",
    "/register/dev": "设备注册",
    "/user/cloud": "用户云盘歌曲列表",
    "/user/cloud/url": "用户云盘歌曲播放地址",
    "/user/detail": "用户详情",
    "/user/follow": "关注用户",
    "/user/follow/message": "用户关注消息",
    "/user/history": "用户历史（听歌/播放）",
    "/user/listen": "用户在听",
    "/user/playlist": "用户歌单",
    "/user/video/collect": "用户收藏的视频",
    "/user/video/love": "用户喜欢的视频",
    "/user/vip/detail": "用户 VIP 详情",
    "/video/detail": "视频详情",
    "/video/privilege": "视频权限",
    "/video/url": "视频播放地址",
    "/ai/recommend": "AI 推荐",
    "/server/now": "服务器时间",
}

GROUPS = [
    ("搜索", ["search", "lyric"]),
    ("歌曲与音频", ["song", "audio", "krm", "kmr", "privilege"]),
    ("专辑", ["album"]),
    ("歌手", ["artist", "singer"]),
    ("歌单（sheet/playlist）", ["sheet", "playlist"]),
    ("排行榜", ["rank", "top/song", "top/album", "top/ip", "top/playlist"]),
    ("评论", ["comment"]),
    ("电台与 FM", ["fm", "pc/diantai", "personal/fm"]),
    ("推荐与首页", ["recommend", "everyday", "brush", "top/card", "images", "yueku", "scene", "theme", "ai"]),
    ("登录与设备", ["login", "captcha", "register"]),
    ("用户", ["user"]),
    ("视频", ["video"]),
    ("长音频", ["longaudio"]),
    ("青年模式（youth）", ["youth"]),
    ("组队活动", ["team"]),
    ("其他", []),
]

def group_of(route):
    s = route.strip("/")
    for name, prefixes in GROUPS:
        for p in prefixes:
            if s == p or s.startswith(p + "/") or s.startswith(p):
                # 精确前缀匹配：避免 "playlist" 吃掉 "sheet" 之类不会发生；注意 top 分组顺序
                return name
    return "其他"

# 实测确认需要登录态/账号参数才能返回有效数据的接口（在代码判定基础上补充）
LOGIN_EXTRA = {
    "/user/detail", "/user/vip/detail", "/user/cloud/url",
    "/playlist/track/all/new", "/youth/channel/all", "/artist/follow/newsongs",
    "/login", "/login/cellphone", "/login/token", "/login/device",
    "/login/device/kick", "/login/openplat", "/login/wx/check",
}

# 特殊说明（显示在接口描述下方）
NOTES = {
    "/playlist/track/all": "`id` 需为歌单的 `global_collection_id`（可从 /top/playlist 返回的 `special_list[].global_collection_id` 获取）。",
    "/playlist/detail": "`ids` 支持逗号分隔多个 `global_collection_id`。",
    "/comment/floor": "`tid` 为评论 ID（可从 /comment/music 返回的 `list[].id` 获取）；实测传入的 tid 与 mixsongid 不匹配时上游返回「歌曲不存在」。",
    "/lyric": "通常需配合 /song/url 返回的 `id`/`accesskey` 使用；直接传 hash 实测返回 400 Bad Request。",
    "/top/card/youth": "实测上游请求 URL 拼接异常（`https://gateway.kugou.comyouth/...`，疑似 host+router 拼接问题），待排查。",
    "/song/url": "实测本机 IP 触发酷狗风控（errcode 20028「本次请求需要验证」），正常客户端环境下应返回播放地址。",
    "/song/url/new": "同 /song/url，实测触发风控（check r failed）。",
    "/video/url": "需传入真实的 MV/视频 hash（歌曲 hash 无效，实测返回 Not found）。",
    "/search": "实测默认 `type=song`（走上游 `/v3/search/song`）当前返回 error_code 152、结果为空（与 Node 版行为一致）；`type=album/mv/author/special/lyric` 正常。",
}

def py_type(v):
    if isinstance(v, bool): return "boolean"
    if isinstance(v, (int, float)): return "number"
    if isinstance(v, str): return "string"
    if isinstance(v, list): return "array"
    if isinstance(v, dict): return "object"
    if v is None: return "null"
    return type(v).__name__

def schema(v, depth=0, max_depth=2):
    """返回 [(字段路径, 类型, 说明)]，说明对数组取首元素结构。"""
    out = []
    if depth >= max_depth:
        return out
    if isinstance(v, dict):
        for k, val in v.items():
            t = py_type(val)
            note = ""
            if isinstance(val, list) and val and isinstance(val[0], (dict, list)):
                inner = schema(val[0], depth + 1, max_depth)
                if inner:
                    note = "array<object>，元素字段见下"
            out.append((k, t, note))
            if isinstance(val, dict) and depth + 1 < max_depth:
                for k2, t2, n2 in schema(val, depth + 1, max_depth):
                    out.append(("  " * (depth + 1) + "└ " + k2, t2, n2))
            elif isinstance(val, list) and val and isinstance(val[0], dict) and depth + 1 < max_depth:
                for k2, t2, n2 in schema(val[0], depth + 1, max_depth):
                    out.append(("  " * (depth + 1) + "└ []." + k2, t2, n2))
    return out

def trim_example(obj, limit=1600):
    s = json.dumps(obj, ensure_ascii=False, indent=2)
    if len(s) <= limit:
        return s
    return s[:limit] + "\n  ...(已截断，完整数据见 responses/)"

def main():
    with open(os.path.join(ROOT, "params.json"), encoding="utf-8") as f:
        params = {r["route"]: r for r in json.load(f)["routes"]}
    with open(os.path.join(ROOT, "manifest.json"), encoding="utf-8") as f:
        manifest = {m["route"]: m for m in json.load(f)}

    by_group = {}
    for route in params:
        by_group.setdefault(group_of(route), []).append(route)

    lines = []
    ap = lines.append
    ap("# MoeKoeMusic rust-api 接口文档")
    ap("")
    ap("> 基于源码 `rust-api/src/modules.rs`（160 条路由）整理。请求参数取自代码实现；")
    ap("> 返回参数为本地真实调用采集（`rust-api/.api-doc/responses/` 保存有完整 JSON）。")
    ap("> 标注「需登录」的接口暂留空，待取得登录态后补充返回结构。")
    ap("")
    ap("- 基础地址：`http://127.0.0.1:3000`（默认端口，可用 `--port` 修改）")
    ap("- 请求方式：所有接口同时支持 `GET`（query 传参）与 `POST`（JSON body 传参）")
    ap("- 登录态：通过 query `cookie` / 请求头 `Cookie` / `authorization` 传入（token、userid 等）")
    ap("- 统一错误信封：上游失败返回 `502 {\"status\":0, \"msg\":\"...\"}`")
    ap("- 缓存：成功响应缓存 120s，可用请求头 `x-apicache-bypass: 1` 跳过")
    ap("")
    ap("## 目录")
    ap("")
    total = 0
    for name, _ in GROUPS:
        rs = sorted(by_group.get(name, []))
        if not rs: continue
        total += len(rs)
        ap(f"- **{name}**（{len(rs)}）：`" + "`、`".join(rs) + "`")
    ap(f"\n共 {total} 个接口。")
    ap("")

    for name, _ in GROUPS:
        rs = sorted(by_group.get(name, []))
        if not rs: continue
        ap(f"## {name}\n")
        for route in rs:
            info = params[route]
            m = manifest.get(route, {})
            ap(f"### {route}")
            ap("")
            ap(DESC.get(route, ""))
            ap("")
            if route in NOTES:
                ap("> " + NOTES[route])
                ap("")
            # 请求参数表
            ps = info.get("params") or []
            if ps:
                ap("**请求参数**\n")
                ap("| 参数 | 类型 | 必填 | 默认值 | 说明 |")
                ap("| --- | --- | --- | --- | --- |")
                for p in ps:
                    ap("| {} | {} | {} | {} | {} |".format(
                        p["name"], p.get("type", "string"),
                        "是" if p.get("required") else "否",
                        p.get("default", "") or "-",
                        p.get("desc", "")))
                ap("")
            else:
                ap("**请求参数**：无\n")
            # 登录
            if info.get("needLogin") or route in LOGIN_EXTRA:
                ap("**需登录**：是（依赖 cookie 中的 token/userid 等登录态）\n")
            # 响应
            ap("**返回数据**\n")
            if info.get("needLogin") or route in LOGIN_EXTRA:
                ap("> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。\n")
                continue
            fn = os.path.join(ROOT, "responses", m.get("file", ""))
            ok = m.get("ok") and os.path.exists(fn)
            if not ok:
                err = m.get("error") or ("HTTP " + str(m.get("http_status")))
                ap(f"> 采集未成功（{err}），返回结构待补。\n")
                continue
            try:
                data = json.load(open(fn, encoding="utf-8"))
            except Exception:
                ap("> 返回非 JSON，待补。\n")
                continue
            if not isinstance(data, dict):
                data = {"data": data}
            biz = data.get("status", data.get("error_code", data.get("errcode")))
            biz_ok = biz == 1 or (biz == 0 and isinstance(data.get("error_code"), int)
                                  and data.get("error_code") == 0 and "status" not in data)
            if biz_ok:
                ap(f"实测返回（业务码 `status={biz}`）：\n")
            else:
                ap(f"> 实测返回为业务错误（业务码 `{biz}`），以下为上游真实错误信封；成功结构待在有效参数/环境下补充：\n")
            sch = schema(data)
            if sch:
                ap("**返回字段结构**（基于真实返回推导）\n")
                ap("```")
                for k, t, note in sch:
                    ap(f"{k}: {t}" + (f"  // {note}" if note else ""))
                ap("```\n")
            ap("**返回示例**（截断）\n")
            ap("```json")
            ap(trim_example(data))
            ap("```\n")

    os.makedirs(os.path.dirname(DOCS_OUT), exist_ok=True)
    open(DOCS_OUT, "w", encoding="utf-8").write("\n".join(lines))
    print("written", DOCS_OUT, len(lines), "lines")

if __name__ == "__main__":
    main()
