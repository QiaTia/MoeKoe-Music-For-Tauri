# MoeKoeMusic rust-api 接口文档

> 基于源码 `rust-api/src/modules.rs`（160 条路由）整理。请求参数取自代码实现；
> 返回参数为本地真实调用采集（`rust-api/.api-doc/responses/` 保存有完整 JSON）。
> 标注「需登录」的接口暂留空，待取得登录态后补充返回结构。

- 基础地址：`http://127.0.0.1:3000`（默认端口，可用 `--port` 修改）
- 请求方式：所有接口同时支持 `GET`（query 传参）与 `POST`（JSON body 传参）
- 登录态：通过 query `cookie` / 请求头 `Cookie` / `authorization` 传入（token、userid 等）
- 统一错误信封：上游失败返回 `502 {"status":0, "msg":"..."}`
- 缓存：成功响应缓存 120s，可用请求头 `x-apicache-bypass: 1` 跳过

## 目录

- **搜索**（8）：`/lyric`、`/search`、`/search/complex`、`/search/default`、`/search/hot`、`/search/lyric`、`/search/mixed`、`/search/suggest`
- **歌曲与音频**（12）：`/audio`、`/audio/accompany/matching`、`/audio/ktv/total`、`/audio/related`、`/kmr/audio/mv`、`/krm/audio`、`/privilege/lite`、`/song/climax`、`/song/ranking`、`/song/ranking/filter`、`/song/url`、`/song/url/new`
- **专辑**（4）：`/album`、`/album/detail`、`/album/shop`、`/album/songs`
- **歌手**（10）：`/artist/albums`、`/artist/audios`、`/artist/detail`、`/artist/follow`、`/artist/follow/newsongs`、`/artist/honour`、`/artist/lists`、`/artist/unfollow`、`/artist/videos`、`/singer/list`
- **歌单（sheet/playlist）**（15）：`/playlist/add`、`/playlist/del`、`/playlist/detail`、`/playlist/effect`、`/playlist/similar`、`/playlist/tags`、`/playlist/track/all`、`/playlist/track/all/new`、`/playlist/tracks/add`、`/playlist/tracks/del`、`/sheet/collection`、`/sheet/collection/detail`、`/sheet/detail`、`/sheet/hot`、`/sheet/list`
- **排行榜**（9）：`/rank/audio`、`/rank/info`、`/rank/list`、`/rank/top`、`/rank/vol`、`/top/album`、`/top/ip`、`/top/playlist`、`/top/song`
- **评论**（7）：`/comment/album`、`/comment/count`、`/comment/floor`、`/comment/music`、`/comment/music/classify`、`/comment/music/hotword`、`/comment/playlist`
- **电台与 FM**（6）：`/fm/class`、`/fm/image`、`/fm/recommend`、`/fm/songs`、`/pc/diantai`、`/personal/fm`
- **推荐与首页**（26）：`/ai/recommend`、`/brush`、`/everyday/friend`、`/everyday/history`、`/everyday/recommend`、`/everyday/style/recommend`、`/images`、`/images/audio`、`/recommend/songs`、`/scene/audio/list`、`/scene/collection/list`、`/scene/lists`、`/scene/lists/v2`、`/scene/module`、`/scene/module/info`、`/scene/music`、`/scene/video/list`、`/theme/music`、`/theme/music/detail`、`/theme/playlist`、`/theme/playlist/track`、`/top/card`、`/top/card/youth`、`/yueku`、`/yueku/banner`、`/yueku/fm`
- **登录与设备**（13）：`/captcha/sent`、`/login`、`/login/cellphone`、`/login/device`、`/login/device/kick`、`/login/openplat`、`/login/qr/check`、`/login/qr/create`、`/login/qr/key`、`/login/token`、`/login/wx/check`、`/login/wx/create`、`/register/dev`
- **用户**（11）：`/user/cloud`、`/user/cloud/url`、`/user/detail`、`/user/follow`、`/user/follow/message`、`/user/history`、`/user/listen`、`/user/playlist`、`/user/video/collect`、`/user/video/love`、`/user/vip/detail`
- **视频**（3）：`/video/detail`、`/video/privilege`、`/video/url`
- **长音频**（6）：`/longaudio/album/audios`、`/longaudio/album/detail`、`/longaudio/daily/recommend`、`/longaudio/rank/recommend`、`/longaudio/vip/recommend`、`/longaudio/week/recommend`
- **青年模式（youth）**（16）：`/youth/channel/all`、`/youth/channel/amway`、`/youth/channel/detail`、`/youth/channel/similar`、`/youth/channel/song`、`/youth/channel/song/detail`、`/youth/channel/sub`、`/youth/day/vip`、`/youth/day/vip/upgrade`、`/youth/dynamic`、`/youth/dynamic/recent`、`/youth/listen/song`、`/youth/month/vip/record`、`/youth/union/vip`、`/youth/user/song`、`/youth/vip`
- **组队活动**（5）：`/team/join`、`/team/my`、`/team/my/info`、`/team/my/status`、`/team/period/info`
- **其他**（9）：`/favorite/count`、`/ip`、`/ip/dateil`、`/ip/playlist`、`/ip/zone`、`/ip/zone/home`、`/lastest/songs/listen`、`/playhistory/upload`、`/server/now`

共 160 个接口。

## 搜索

### /lyric

获取歌词（含 KRC 解密，翻译/罗马音）

> 通常需配合 /song/url 返回的 `id`/`accesskey` 使用；直接传 hash 实测返回 400 Bad Request。

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | string | 是 | - | 歌词/歌曲ID |
| accesskey | string | 否 | - | 访问密钥 |
| client | string | 否 | android | 客户端类型 |
| fmt | string | 否 | krc | 歌词格式 krc/lrc |
| decode | boolean | 否 | false | 是否解码歌词 |

**返回数据**

> 实测返回为业务错误（业务码 `400`），以下为上游真实错误信封；成功结构待在有效参数/环境下补充：

**返回字段结构**（基于真实返回推导）

```
charset: string
content: string
fmt: string
info: string
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "charset": "",
  "content": "",
  "fmt": "",
  "info": "Bad Request",
  "status": 400,
  "error_code": 20010
}
```

### /search

歌曲搜索

> 实测默认 `type=song`（走上游 `/v3/search/song`）当前返回 error_code 152、结果为空（与 Node 版行为一致）；`type=album/mv/author/special/lyric` 正常。

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| keywords | string | 是 | - | 搜索关键词 |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |
| type | string | 否 | song | 搜索类型 special/lyric/song/album/author/mv |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
status: number
error_code: number
error_msg: string
data: object
  └ pagesize: number
  └ page: number
  └ from: number
  └ size: number
  └ total: number
  └ correctiontype: number
  └ correctionforce: number
  └ correctiontip: string
  └ lists: array
```

**返回示例**（截断）

```json
{
  "status": 1,
  "error_code": 0,
  "error_msg": "",
  "data": {
    "pagesize": 3,
    "page": 1,
    "from": 0,
    "size": 3,
    "total": 500,
    "correctiontype": 0,
    "correctionforce": 0,
    "correctiontip": "",
    "lists": [
      {
        "albumid": 45305792,
        "albumname": "一路生花",
        "singer": "温奕心",
        "singerid": "0",
        "grade": 0,
        "grade_int": "",
        "img": "http://imge.kugou.com/stdmusic/240/20251016/20251016152611214074.jpg",
        "intro": "",
        "grade_float": "",
        "company": "",
        "quality": 2,
        "title": "一路生花 - 温奕心",
        "collect_count": 0,
        "publish_time": "2021-01-19",
        "language": "华语",
        "privilege": 0,
        "oldhide": 0,
        "buyercount": 0,
        "songcount": 6,
        "newquality": 1,
        "cd_url": "",
        "isfirst": 0,
        "category": 1,
        "short_intro": "",
        "ostremark": "",
        "auxiliary": "",
        "chapter_name": "",
        "play_times": 0,
        "program_inner": 0,
        "alg_path": "",
        "program_def_songs": [],
        "tag_str": "",
        "album_aux": "",
        "play_count": 0,
        "isouter": 0,
        "outerdata": {},
        "trans_param": {
          "special_tag": "54607872"
        },
        "special_tag": "54607872",
        "singerids": [
          532327
        ],
        "singers": [
          {
            "name": "温奕心",
            "id": 532327
          }
        ],
        "rec_tag": {
          "desc": "",
          "type": 0
        },
        "jump_url": ""
      },
  
  ...(已截断，完整数据见 responses/)
```

### /search/complex

综合搜索（多类型聚合结果）

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| keywords | string | 是 | - | 搜索关键词 |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |

**返回数据**

> 实测返回为业务错误（业务码 `None`），以下为上游真实错误信封；成功结构待在有效参数/环境下补充：

**返回字段结构**（基于真实返回推导）

```
data: string
```

**返回示例**（截断）

```json
{
  "data": "<!--KG_TAG_RES_START-->{\n  \"status\": 1,\n  \"error_code\": 152,\n  \"error_msg\": \"Parameter Error\",\n  \"data\": {\n    \"correctionforce\":0,\n    \"correctiontip\":\"\",\n    \"correctiontype\":0,\n    \"indextotal\":8,\n    \"lists\": [\n      {\n        \"type\": \"song\",\n        \"total\": 0,\n        \"allowerr\": 0,\n        \"index\": 1,\n        \"isshareresult\": 0,\n        \"sectag_info\":{\n            \"is_sectag\": 0\n        },\n        \"lists\": []\n      },\n      {\n        \"type\": \"mv\",\n        \"total\": 0,\n        \"index\": 2,\n        \"lists\": []\n      },\n      {\n        \"type\": \"collect\",\n        \"total\": 0,\n        \"index\": 3,\n        \"lists\": []\n      },\n      {\n        \"type\": \"album\",\n        \"total\": 0,\n        \"index\": 4,\n        \"lists\": []\n      },\n      {\n        \"type\": \"ksong\",\n        \"total\": 0,\n        \"index\": 5,\n        \"lists\": []\n      },\n      {\n        \"type\": \"program\",\n        \"total\": 0,\n        \"index\": 6,\n        \"lists\": []\n      },\n      {\n        \"type\": \"author\",\n        \"total\": 0,\n        \"index\": 7,\n        \"lists\": []\n      },\n      {\n        \"type\": \"talent\",\n        \"total\": 0,\n        \"index\": 8,\n        \"lists\": []\n      }\n    ]\n  }\n}\n<!--KG_TAG_RES_END-->\n"
}
```

### /search/default

搜索默认词（打开搜索页时的推荐词）

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| vip_type | number | 否 | 65530 | 会员类型 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
errcode: number
errmsg: string
data: object
  └ timestamp: number
  └ ads: array
  └ fallback: null
status: number
```

**返回示例**（截断）

```json
{
  "errcode": 0,
  "errmsg": "",
  "data": {
    "timestamp": 1788516861,
    "ads": [
      {
        "abs_pos": 2,
        "end_time": "2026-11-01 23:59:59",
        "id": 449561,
        "is_preview": 0,
        "jumpType": "EmbededPage",
        "main_title": "免费听歌",
        "show_times": 0,
        "start_time": "2026-07-17 07:00:00",
        "sub_title": "",
        "title": "免费听歌",
        "type": 1,
        "unifiedUrl": "eyJjbGlja1RyYWNrIjoiIiwiZGVlcExpbmsiOiIiLCJkaXNwbGF5QmFyIjowLCJqdW1wVXJsIjoiaHR0cHM6Ly9hY3Rpdml0eS5rdWdvdS5jb20vdm9vL3YtYjU1MTA2N2MvaW5kZXguaHRtbD9jbGllbnRUeXBlPWt1Z291XHUwMDI2aHJlZmZyb209bm9mb2N1c3NlYXJjaF9zb25nIiwidGl0bGUiOiLlhY3otLnlkKzmrYwifQ=="
      }
    ],
    "fallback": null
  },
  "status": 1
}
```

### /search/hot

热门搜索词

**请求参数**：无

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
status: number
errcode: number
data: object
  └ timestamp: number
  └ list: array
error: string
```

**返回示例**（截断）

```json
{
  "status": 1,
  "errcode": 0,
  "data": {
    "timestamp": 1788516861,
    "list": [
      {
        "name": "热搜榜",
        "keywords": [
          {
            "reason": "甲乙丙丁",
            "json_url": "",
            "jumpurl": "",
            "keyword": "甲乙丙丁",
            "is_cover_word": 0,
            "type": 1,
            "icon": 0
          },
          {
            "reason": "我不难过",
            "json_url": "",
            "jumpurl": "",
            "keyword": "我不难过",
            "is_cover_word": 0,
            "type": 1,
            "icon": 0
          },
          {
            "reason": "山风山风等等我",
            "json_url": "",
            "jumpurl": "",
            "keyword": "山风山风等等我",
            "is_cover_word": 0,
            "type": 1,
            "icon": 0
          },
          {
            "reason": "如果累了就回故乡",
            "json_url": "",
            "jumpurl": "",
            "keyword": "如果累了就回故乡",
            "is_cover_word": 0,
            "type": 1,
            "icon": 0
          },
          {
            "reason": "你有没有真的爱过我",
            "json_url": "",
            "jumpurl": "",
            "keyword": "你有没有真的爱过我",
            "is_cover_word": 0,
            "type": 1,
            "icon": 0
          },
          {
            "reason": "泪海",
            "json_url": "",
            "jumpurl": "",
            "keyword": "泪海",
            "is_cover_word": 0,
            "type": 1,
            "icon": 0
          },
          {
            "reason": "今生劫换来生缘",
            "json_url": "",
            "jumpurl": "",
            "keyword": "今生劫换来生缘
  ...(已截断，完整数据见 responses/)
```

### /search/lyric

歌词搜索（按歌词内容找歌）

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| hash | string | 是 | - | 歌曲hash |
| keywords | string | 是 | - | 歌词搜索关键词 |
| album_audio_id | number | 否 | 0 | 音频ID |
| man | string | 否 | no | 是否人工 |

**返回数据**

> 实测返回为业务错误（业务码 `200`），以下为上游真实错误信封；成功结构待在有效参数/环境下补充：

**返回字段结构**（基于真实返回推导）

```
status: number
info: string
errcode: number
errmsg: string
keyword: string
proposal: string
has_complete_right: number
companys: string
ugc: number
ugccount: number
expire: number
candidates: array  // array<object>，元素字段见下
  └ [].id: string
  └ [].product_from: string
  └ [].accesskey: string
  └ [].can_score: boolean
  └ [].singer: string
  └ [].song: string
  └ [].duration: number
  └ [].uid: string
  └ [].nickname: string
  └ [].origiuid: string
  └ [].transuid: string
  └ [].sounduid: string
  └ [].originame: string
  └ [].transname: string
  └ [].soundname: string
  └ [].parinfo: array
  └ [].parinfoExt: array
  └ [].language: string
  └ [].krctype: number
  └ [].hitlayer: number
  └ [].hitcasemask: number
  └ [].adjust: number
  └ [].score: number
  └ [].contenttype: number
  └ [].content_format: number
  └ [].download_id: string
ugccandidates: array
artists: array
ai_candidates: array
```

**返回示例**（截断）

```json
{
  "status": 200,
  "info": "OK",
  "errcode": 200,
  "errmsg": "OK",
  "keyword": "周杰伦",
  "proposal": "61641844",
  "has_complete_right": 0,
  "companys": "",
  "ugc": 0,
  "ugccount": 0,
  "expire": 7200,
  "candidates": [
    {
      "id": "61641844",
      "product_from": "ugc",
      "accesskey": "A73A863722BB52E47E3175CF3B8C7F28",
      "can_score": true,
      "singer": "周杰伦",
      "song": "周杰伦",
      "duration": 32626,
      "uid": "928165386",
      "nickname": "妖怪",
      "origiuid": "0",
      "transuid": "0",
      "sounduid": "0",
      "originame": "",
      "transname": "",
      "soundname": "",
      "parinfo": [
        [
          928165386,
          "妖怪",
          "2021/12/11 15:11",
          "http://imge.kugou.com/kugouicon/165/20230608/20230608162228517118.jpg"
        ]
      ],
      "parinfoExt": [
        {
          "entry": "0"
        }
      ],
      "language": "",
      "krctype": 2,
      "hitlayer": 7,
      "hitcasemask": 12,
      "adjust": 0,
      "score": 60,
      "contenttype": 0,
      "content_format": 1,
      "download_id": "61641844"
    }
  ],
  "ugccandidates": [],
  "artists": [],
  "ai_candidates": []
}
```

### /search/mixed

混合搜索（专辑/歌手/歌单/用户等聚合）

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| keyword | string | 是 | - | 搜索关键词 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
status: number
error_code: number
error_msg: string
data: object
  └ correctionforce: number
  └ correctiontip: string
  └ correctiontype: number
  └ lists: array
took: number
```

**返回示例**（截断）

```json
{
  "status": 1,
  "error_code": 152,
  "error_msg": "Parameter Error",
  "data": {
    "correctionforce": 0,
    "correctiontip": "",
    "correctiontype": 0,
    "lists": []
  },
  "took": 9223372036854
}
```

### /search/suggest

搜索建议（输入联想）

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| keywords | string | 是 | - | 搜索关键词 |
| albumTipCount | number | 否 | 10 | 专辑提示数 |
| correctTipCount | number | 否 | 10 | 纠错提示数 |
| mvTipCount | number | 否 | 10 | MV提示数 |
| musicTipCount | number | 否 | 10 | 歌曲提示数 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
status: number
error_code: number
ErrorCode: number
data: array  // array<object>，元素字段见下
  └ [].RecordDatas: array
  └ [].RecordCount: number
  └ [].LableName: string
SingerShortcut: object
  └ id: number
  └ name: string
  └ img: string
  └ song_count: number
  └ album_count: number
  └ fans_count: number
Shortcuts: null
```

**返回示例**（截断）

```json
{
  "status": 1,
  "error_code": 0,
  "ErrorCode": 0,
  "data": [
    {
      "RecordDatas": [
        {
          "HintInfo": "周杰伦",
          "MatchCount": 0,
          "Hot": 9908,
          "IsRadio": 0,
          "IsKlist": 1,
          "Use": "klist_tip_standard_v3_",
          "scores": 0.4157977104187012,
          "la": 0,
          "icon": 0,
          "subtitle": "",
          "busi": "",
          "complex_ai_action": 0
        },
        {
          "HintInfo": "周杰伦 太阳之子",
          "MatchCount": 0,
          "Hot": 145,
          "IsRadio": 0,
          "IsKlist": 1,
          "Use": "klist_tip_standard_v3_",
          "scores": 0.013632787391543388,
          "la": 0,
          "icon": 0,
          "subtitle": "",
          "busi": "",
          "complex_ai_action": 0
        },
        {
          "HintInfo": "周杰伦 告白气球",
          "MatchCount": 0,
          "Hot": 96,
          "IsRadio": 0,
          "IsKlist": 1,
          "Use": "klist_tip_standard_v3_",
          "scores": 0.00397855369374156,
          "la": 0,
          "icon": 0,
          "subtitle": "",
          "busi": "",
          "complex_ai_action": 0
        },
        {
          "HintInfo": "周杰伦 那天下雨了",
          "MatchCount": 0,
          "Hot": 57,
          "IsRadio": 0,
          "IsKlist": 1,
          "Use": "klist_tip_standard_v3_",
          "scores": 0.003249571891501546,
          "la": 0,
          "icon": 0,
          "subtitle": "",
          "busi": "",
          "complex_ai_action": 0
        },
        {
          "HintInfo": "周杰伦最新演唱会动态",
          "MatchCount": 0,
        
  ...(已截断，完整数据见 responses/)
```

## 歌曲与音频

### /audio

音频（歌曲）详情

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| hash | string | 是 | - | 歌曲hash，逗号分隔 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
status: number
error_code: number
errcode: number
errmsg: string
data: array
```

**返回示例**（截断）

```json
{
  "status": 1,
  "error_code": 0,
  "errcode": 0,
  "errmsg": "",
  "data": [
    {}
  ]
}
```

### /audio/accompany/matching

伴奏匹配

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| fileName | string | 否 | - | 文件名 |
| hash | string | 否 | - | 歌曲hash |
| mixId | number | 否 | 0 | 混音ID |

**返回数据**

> 实测返回为业务错误（业务码 `None`），以下为上游真实错误信封；成功结构待在有效参数/环境下补充：

**返回字段结构**（基于真实返回推导）

```
code: number
msg: string
times: number
data: string
```

**返回示例**（截断）

```json
{
  "code": 2,
  "msg": "请求的参数不符合加密要求.",
  "times": 0,
  "data": "{\"fileName\":[\"\"],\"isteen\":[\"0\"],\"mixId\":[\"0\"],\"appid\":[\"1005\"],\"sign\":[\"fedce0274563ab10\"],\"usemkv\":[\"1\"],\"version\":[\"12375\"],\"platform\":[\"2\"],\"hash\":[\"e291fc24c9dbe4ef3fdd9837ea3e1a9c\"]}"
}
```

### /audio/ktv/total

KTV 演唱次数统计

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| songId | number | 否 | 0 | 歌曲ID |
| singerName | string | 否 | - | 歌手名 |
| songHash | string | 否 | - | 歌曲hash |

**返回数据**

> 实测返回为业务错误（业务码 `None`），以下为上游真实错误信封；成功结构待在有效参数/环境下补充：

**返回字段结构**（基于真实返回推导）

```
code: number
msg: string
times: number
data: string
```

**返回示例**（截断）

```json
{
  "code": 2,
  "msg": "请求的参数不符合加密要求.",
  "times": 0,
  "data": "{\"singerName\":[\"\"],\"isteen\":[\"0\"],\"appid\":[\"1005\"],\"sign\":[\"7aaf8e476c979331\"],\"usemkv\":[\"1\"],\"songId\":[\"0\"],\"version\":[\"12375\"],\"platform\":[\"2\"],\"songHash\":[\"\"]}"
}
```

### /audio/related

相关音频推荐

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| album_audio_id | number | 否 | 0 | 音频ID |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |
| show_type | number | 否 | 0 | 显示类型 |
| sort | string | 否 | all | 排序 all/hot/new |
| type | number | 否 | 0 | 类型 |
| show_detail | number | 否 | 0 | 是否仅统计 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
status: number
error_code: number
errmsg: string
total: number
extra: object
  └ page_total: number
  └ original: array
  └ resp: object
data: array  // array<object>，元素字段见下
  └ [].base: object
  └ [].extra: object
  └ [].audio_info: object
  └ [].album_info: object
  └ [].authors: array
  └ [].landscape_mv: object
  └ [].obbligato: object
  └ [].works: array
  └ [].copyright: object
  └ [].hot_flag: number
```

**返回示例**（截断）

```json
{
  "status": 1,
  "error_code": 0,
  "errmsg": "",
  "total": 1986,
  "extra": {
    "page_total": 1986,
    "original": [
      {
        "base": {
          "album_audio_id": 317568292,
          "audio_group_id": 138820721,
          "audio_id": 92351310,
          "album_id": 45305792,
          "songname": "一路生花",
          "author_name": "温奕心",
          "album_name": "一路生花",
          "is_publish": 1,
          "version": 1,
          "publish_date": "2021-01-19"
        },
        "extra": {
          "is_original": 1,
          "mixsong_type": 0
        },
        "audio_info": {
          "hash": "4F18553F2AF06E355AD75A5EE6173456",
          "filesize": 4101213,
          "timelength": 256286,
          "bitrate": 128,
          "hash_320": "A2E0F7D305D5C9F778CA19335550E3E0",
          "filesize_320": 10252736,
          "timelength_320": 256286,
          "hash_flac": "6980080CCFF5BC13CB515121687BFBCC",
          "filesize_flac": 28321807,
          "timelength_flac": 256250,
          "bitrate_flac": 883,
          "hash_high": "E7BC5053D50B2DA3CD039466CCB93885",
          "filesize_high": 51245499,
          "timelength_high": 256245,
          "bitrate_high": 1589,
          "hash_super": "",
          "filesize_super": 0,
          "timelength_super": 0,
          "bitrate_super": 0,
          "extname": "mp3",
          "extname_super": ""
        },
        "album_info": {
          "cover": "http://imge.kugou.com/stdmusic/{size}/20251016/20251016152611214074.jpg"
        },
        "authors": [
          {
            "base": {
              "author_id": 
  ...(已截断，完整数据见 responses/)
```

### /kmr/audio/mv

KMR 音频关联 MV

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| album_audio_id | string | 是 | - | 音频ID，逗号分隔 |
| fields | string | 否 | - | 返回字段 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
status: number
error_code: number
errmsg: string
data: array
```

**返回示例**（截断）

```json
{
  "status": 1,
  "error_code": 0,
  "errmsg": "",
  "data": [
    [
      {
        "is_recommend": 1,
        "user_id": 0,
        "hot": 1732,
        "hit": 23375254,
        "user_name": "",
        "songid": 138820721,
        "thumb": "http://imge.kugou.com/mvthumb/{size}/75/46/7546b6d6ce64a2691de1761e42b0ee1f.jpg",
        "desc": "",
        "__status": 1,
        "singer": "温奕心",
        "other_desc": "",
        "hdpic": "http://imge.kugou.com/mvhdpic/{size}/20220117/20220117141651950941.jpg",
        "is_ugc": 0,
        "have_mp4": 1,
        "audio_id": 92351310,
        "album_audio_id": 317568292,
        "is_publish": 1,
        "download_total": 514479,
        "collection_total": 257978,
        "is_short": 0,
        "remark": "",
        "video_id": 5515877,
        "user_avatar": "",
        "music_trac": 3,
        "mv_name": "一路生花",
        "duration": 256880,
        "topic": "",
        "type": 0,
        "is_other": 0,
        "publish_time": "2022-01-14 18:00:00",
        "play_times": 23375254
      },
      {
        "is_recommend": 0,
        "user_id": 0,
        "hot": 1076,
        "hit": 1837231,
        "user_name": "",
        "songid": 138820721,
        "thumb": "http://imge.kugou.com/mvthumb/{size}/cb/92/cb92b71f496ed746347cbb069467768f.jpg",
        "desc": "",
        "__status": 1,
        "singer": "温奕心",
        "other_desc": "",
        "hdpic": "http://imge.kugou.com/mvhdpic/{size}/20240205/20240205155353546518.jpg",
        "is_ugc": 0,
        "have_mp4": 1,
        "audio_id": 92351310,
        "album_audio_id": 317568292,
  ...(已截断，完整数据见 responses/)
```

### /krm/audio

KRM 音频信息

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| album_audio_id | string | 是 | - | 音频实体ID，逗号分隔 |
| fields | string | 否 | base | 返回字段 base |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
status: number
error_code: number
msg: string
data: array  // array<object>，元素字段见下
  └ [].__status: number
```

**返回示例**（截断）

```json
{
  "status": 1,
  "error_code": 0,
  "msg": "",
  "data": [
    {
      "__status": 1
    }
  ]
}
```

### /privilege/lite

歌曲权限（精简版）

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| hash | string | 是 | - | 歌曲hash(逗号分隔) |
| album_id | string | 否 | - | 专辑ID(逗号分隔) |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
status: number
error_code: number
message: string
appid_group: number
should_cache: number
data: array  // array<object>，元素字段见下
  └ [].type: string
  └ [].id: number
  └ [].album_id: string
  └ [].recommend_album_id: string
  └ [].album_audio_id: number
  └ [].hash: string
  └ [].name: string
  └ [].singername: string
  └ [].level: number
  └ [].quality: string
  └ [].expire: number
  └ [].publish: number
  └ [].is_publish: number
  └ [].old_hide: number
  └ [].privilege: number
  └ [].status: number
  └ [].fail_process: number
  └ [].pay_type: number
  └ [].price: number
  └ [].pkg_price: number
  └ [].topic_url: string
  └ [].topic_remark: string
  └ [].info: object
  └ [].discount: array
  └ [].start_time: string
  └ [].end_time: string
  └ [].cd_url: string
  └ [].cid: number
  └ [].old_cpy: number
  └ [].rebuy_pay_type: number
  └ [].is_search_top: number
  └ [].pay_block_text: string
  └ [].is_separate: number
  └ [].buy_count: number
  └ [].buy_count_vip: number
  └ [].buy_count_kubi: number
  └ [].buy_count_audios: number
  └ [].trans_param: object
  └ []._msg: string
  └ []._errno: number
  └ [].relate_goods: array
```

**返回示例**（截断）

```json
{
  "status": 1,
  "error_code": 0,
  "message": "",
  "appid_group": 1,
  "should_cache": 1,
  "data": [
    {
      "type": "audio",
      "id": 0,
      "album_id": "0",
      "recommend_album_id": "0",
      "album_audio_id": 0,
      "hash": "E291FC24C9DBE4EF3FDD9837EA3E1A9C",
      "name": "",
      "singername": "",
      "level": 0,
      "quality": "",
      "expire": 0,
      "publish": 1,
      "is_publish": 0,
      "old_hide": 0,
      "privilege": 0,
      "status": 1,
      "fail_process": 0,
      "pay_type": 0,
      "price": 0,
      "pkg_price": 0,
      "topic_url": "",
      "topic_remark": "",
      "info": {
        "duration": 0,
        "filesize": 0,
        "bitrate": 0,
        "extname": "",
        "flag": 0,
        "image": "",
        "intro": ""
      },
      "discount": [],
      "start_time": "0000-00-00",
      "end_time": "0000-00-00",
      "cd_url": "",
      "cid": -1,
      "old_cpy": 1,
      "rebuy_pay_type": 0,
      "is_search_top": 0,
      "pay_block_text": "0",
      "is_separate": 0,
      "buy_count": 0,
      "buy_count_vip": 0,
      "buy_count_kubi": 0,
      "buy_count_audios": 0,
      "trans_param": {
        "musicpack_advance": 0,
        "pay_block_tpl": 1,
        "display": 0,
        "display_rate": 0,
        "cid": -1,
        "cpy_attr0": 0,
        "cpy_map": "",
        "classmap": {
          "attr0": 0
        },
        "qualitymap": {
          "attr0": 0,
          "attr1": 0,
          "bits": ""
        }
      },
      "_msg": "Allow: the audio is not found.",
      "_errno": 6,
      "relate_goods
  ...(已截断，完整数据见 responses/)
```

### /song/climax

获取歌曲高潮点（副歌时间段）

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| hash | string | 是 | - | 歌曲hash，逗号分隔 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
status: number
error_code: number
errcode: number
errmsg: string
data: array
```

**返回示例**（截断）

```json
{
  "status": 1,
  "error_code": 0,
  "errcode": 0,
  "errmsg": "",
  "data": [
    {}
  ]
}
```

### /song/ranking

歌曲在排行榜中的排名

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| album_audio_id | string | 是 | - | 歌曲音频ID |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
errcode: number
errmsg: string
data: object
  └ album_audio_id: number
  └ info: array
  └ platform_num: number
  └ platform_total: number
  └ title1: string
  └ title2: string
status: number
```

**返回示例**（截断）

```json
{
  "errcode": 0,
  "errmsg": "",
  "data": {
    "album_audio_id": 317568292,
    "info": [
      {
        "platform": 1,
        "platform_name": "酷狗音乐",
        "platform_logo": "https://mobileservicebssdl.kugou.com/63d7e2a9cae12efb85275cea40615244.png",
        "last_time": "2026-08-26",
        "ranking_num": 10,
        "ranking_times": 3250
      },
      {
        "platform": 2,
        "platform_name": "QQ音乐",
        "platform_logo": "https://mobileservicebssdl.kugou.com/be36e7470c90a346f2e76b08dd6323bb.png",
        "last_time": "2026-03-07",
        "ranking_num": 4,
        "ranking_times": 1584
      },
      {
        "platform": 3,
        "platform_name": "酷我音乐",
        "platform_logo": "https://mobileservicebssdl.kugou.com/f20ff36b92eacb1b35a75c490b13dca6.png",
        "last_time": "2026-09-04",
        "ranking_num": 5,
        "ranking_times": 1668
      },
      {
        "platform": 15,
        "platform_name": "iTunes",
        "platform_logo": "http://mobileservicebssdl.kugou.com/5b59c284cdc22961046c7d514d990043.png",
        "last_time": "2024-06-06",
        "ranking_num": 2,
        "ranking_times": 2
      }
    ],
    "platform_num": 4,
    "platform_total": 17,
    "title1": "已上榜 4/17",
    "title2": "收录全球海内外17个音乐平台上榜信息，该歌曲上榜4个"
  },
  "status": 1
}
```

### /song/ranking/filter

歌曲上榜筛选信息

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| album_audio_id | string | 是 | - | 歌曲音频ID |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |

**返回数据**

> 实测返回为业务错误（业务码 `0`），以下为上游真实错误信封；成功结构待在有效参数/环境下补充：

**返回字段结构**（基于真实返回推导）

```
errcode: number
errmsg: string
data: object
status: number
```

**返回示例**（截断）

```json
{
  "errcode": 20010,
  "errmsg": "参数错误",
  "data": {},
  "status": 0
}
```

### /song/url

获取歌曲播放地址

> 实测本机 IP 触发酷狗风控（errcode 20028「本次请求需要验证」），正常客户端环境下应返回播放地址。

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| hash | string | 是 | - | 歌曲hash |
| album_id | number | 否 | 0 | 专辑ID |
| album_audio_id | number | 否 | 0 | 音频ID |
| quality | string | 否 | 128 | 音质 128/320/flac 或 magic_xxx |
| free_part | boolean | 否 | false | 是否试听片段 |
| ppage_id | string | 否 | - | 分页ID串 |
| notSign | boolean | 否 | false | 拼写错误参数(原Node字段)，实际签名仍计算 |

**返回数据**

> 实测返回为业务错误（业务码 `2`），以下为上游真实错误信封；成功结构待在有效参数/环境下补充：

**返回字段结构**（基于真实返回推导）

```
fail_process: array
errcode: number
status: number
error: string
```

**返回示例**（截断）

```json
{
  "fail_process": [
    "pkg"
  ],
  "errcode": 20028,
  "status": 2,
  "error": "本次请求需要验证"
}
```

### /song/url/new

获取歌曲播放地址（新版）

> 同 /song/url，实测触发风控（check r failed）。

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| hash | string | 是 | - | 歌曲hash |
| album_audio_id | string | 否 | - | 音频ID |
| free_part | boolean | 否 | false | 是否试听片段 |

**返回数据**

> 实测返回为业务错误（业务码 `0`），以下为上游真实错误信封；成功结构待在有效参数/环境下补充：

**返回字段结构**（基于真实返回推导）

```
status: number
error_code: number
message: string
```

**返回示例**（截断）

```json
{
  "status": 0,
  "error_code": 20028,
  "message": "check r failed"
}
```

## 专辑

### /album

专辑信息

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| album_id | string | 是 | - | 专辑ID，逗号分隔 |
| fields | string | 否 | - | 返回字段 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
status: number
error_code: number
errcode: number
errmsg: string
data: array  // array<object>，元素字段见下
  └ [].trans_param: object
  └ [].language: string
  └ [].grade_count: string
  └ [].intro: string
  └ [].heat: string
  └ [].category: string
  └ [].sizable_cover: string
  └ [].cover: string
  └ [].album_name: string
  └ [].type: string
  └ [].quality: string
  └ [].publish_company: string
  └ [].grade: string
  └ [].special_tag: string
  └ [].author_name: string
  └ [].publish_date: string
  └ [].classification: array
  └ [].language_id: string
  └ [].album_id: string
  └ [].exclusive: string
  └ [].is_publish: string
```

**返回示例**（截断）

```json
{
  "status": 1,
  "error_code": 0,
  "errcode": 0,
  "errmsg": "",
  "data": [
    {
      "trans_param": {
        "special_tag": "65536"
      },
      "language": "英语",
      "grade_count": "0",
      "intro": "",
      "heat": "0",
      "category": "1",
      "sizable_cover": "http://imge.kugou.com/stdmusic/{size}/20150902/20150902183311813340.jpg",
      "cover": "20150902183311813340.jpg",
      "album_name": "Hardwood Exits",
      "type": "录音室专辑",
      "quality": "1",
      "publish_company": "",
      "grade": "0",
      "special_tag": "65536",
      "author_name": "Blackbird Harmony",
      "publish_date": "2008-05-13",
      "classification": [
        {
          "usage": "0",
          "status": "0",
          "level": "0",
          "res_type": "2",
          "type": "17",
          "id": "0",
          "res_id": "1289478"
        }
      ],
      "language_id": "2",
      "album_id": "1289478",
      "exclusive": "0",
      "is_publish": "1"
    }
  ]
}
```

### /album/detail

专辑详情

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | string | 是 | - | 专辑ID |
| is_buy | number | 否 | 0 | 是否购买 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
status: number
error_code: number
errcode: number
errmsg: string
data: array  // array<object>，元素字段见下
  └ [].trans_param: object
  └ [].language: string
  └ [].intro: string
  └ [].heat: string
  └ [].sizable_cover: string
  └ [].cover: string
  └ [].album_name: string
  └ [].type: string
  └ [].quality: string
  └ [].author_name: string
  └ [].publish_date: string
  └ [].album_id: string
  └ [].exclusive: string
  └ [].is_publish: string
  └ [].authors: array
```

**返回示例**（截断）

```json
{
  "status": 1,
  "error_code": 0,
  "errcode": 0,
  "errmsg": "",
  "data": [
    {
      "trans_param": {
        "special_tag": "65536"
      },
      "language": "英语",
      "intro": "",
      "heat": "0",
      "sizable_cover": "http://imge.kugou.com/stdmusic/{size}/20150902/20150902183311813340.jpg",
      "cover": "20150902183311813340.jpg",
      "album_name": "Hardwood Exits",
      "type": "录音室专辑",
      "quality": "1",
      "author_name": "Blackbird Harmony",
      "publish_date": "2008-05-13",
      "album_id": "1289478",
      "exclusive": "0",
      "is_publish": "1",
      "authors": [
        {
          "author_id": "509922",
          "author_name": "Blackbird Harmony"
        }
      ]
    }
  ]
}
```

### /album/shop

专辑商城信息（售卖/价格）

**请求参数**：无

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
error_code: number
err_msg: string
data: object
  └ title: string
  └ has_more: number
  └ album_list: array
status: number
err_code: number
```

**返回示例**（截断）

```json
{
  "error_code": 0,
  "err_msg": "success",
  "data": {
    "title": "最新上架",
    "has_more": 1,
    "album_list": [
      {
        "album_name": "CLICK",
        "intro_en": "zj446545",
        "buy_num": 5631,
        "topic_id": 446545,
        "media_id": 205741080,
        "type": 2,
        "singer_name": "JISOO",
        "sale_page_url": "https://ep.kugou.com/html/mobile_commonchargeV3/index_446545.html?is_go=1&hreffrom=45",
        "pic": "https://ep.kugou.com/static/images/commonchargeV3/1788407856/zj446545/album_cover.png",
        "price": 400
      },
      {
        "album_name": "SaWaDiKa",
        "intro_en": "zj446547",
        "buy_num": 0,
        "topic_id": 446547,
        "media_id": 205994129,
        "type": 2,
        "singer_name": "LISA",
        "sale_page_url": "https://ep.kugou.com/html/mobile_commonchargeV3/index_446547.html?is_go=1&hreffrom=45",
        "pic": "https://ep.kugou.com/static/images/commonchargeV3/1788450145/zj446547/album_cover.png",
        "price": 400
      },
      {
        "album_name": "Fallen Angel (Digital EP) [Explicit]",
        "intro_en": "zj446535",
        "buy_num": 7487,
        "topic_id": 446535,
        "media_id": 204983824,
        "type": 2,
        "singer_name": "JENNIE",
        "sale_page_url": "https://ep.kugou.com/html/mobile_commonchargeV3/index_446535.html?is_go=1&hreffrom=45",
        "pic": "https://ep.kugou.com/static/images/commonchargeV3/1787890662/zj446535/album_cover.png",
        "price": 1200
      },
      {
        "album_name": "BLINGY - The 7th Album",
        "intro_en": "zj446490",
 
  ...(已截断，完整数据见 responses/)
```

### /album/songs

专辑内歌曲列表

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | string | 是 | - | 专辑ID |
| is_buy | string | 否 | - | 是否购买 |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
total: number
error_code: number
data: object
  └ total: number
  └ songs: array
extra: object
  └ disc_cnt: number
status: number
errmsg: string
```

**返回示例**（截断）

```json
{
  "total": 8,
  "error_code": 0,
  "data": {
    "total": 8,
    "songs": [
      {
        "mvdata": [],
        "extend": {
          "has_obbligato": 0,
          "cd_name": "",
          "disc": 1,
          "sort": 1
        },
        "trans_param": {
          "cpy_map": "2",
          "pay_block_tpl": 1,
          "classmap": {
            "attr0": 100663304
          },
          "union_cover": "http://imge.kugou.com/stdmusic/{size}/20150902/20150902183311813340.jpg",
          "cid": -1,
          "language": "英语",
          "cpy_attr0": 0,
          "ipmap": {
            "attr0": 137455730752
          },
          "qualitymap": {
            "bits": "14",
            "attr0": 20,
            "attr1": 0
          },
          "musicpack_advance": 0,
          "display": 0,
          "display_rate": 0
        },
        "copyright": {
          "privilege_flac": 0,
          "sale_mode_128_download": 0,
          "viponly_tag": 0,
          "privilege_128": 0,
          "album_sale_url": "",
          "privilege": 0,
          "sale_mode_flac_download": 0,
          "privilege_320": 0,
          "sale_mode_320_download": 0,
          "sale_mode_download": 0
        },
        "audio_info": {
          "hash": "E269A1688E444BA73C733611ABAAFA7F",
          "bitrate": 128,
          "duration_flac": 0,
          "filesize_320": 8392747,
          "filesize_flac": 0,
          "duration_320": 210000,
          "filesize_high": 0,
          "hash_320": "91778813F7D80BDD5544C70E67BCFF0E",
          "duration": 209789,
          "duration_high": 0,
          "hash_hig
  ...(已截断，完整数据见 responses/)
```

## 歌手

### /artist/albums

歌手专辑列表

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | string | 是 | - | 歌手ID |
| sort | string | 否 | 1 | 排序 hot=3 其他=1 |
| pagesize | number | 否 | 30 | 每页数量 |
| page | number | 否 | 1 | 页码 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
total: number
error_code: number
data: array
extra: object
  └ page_total: number
status: number
errmsg: string
```

**返回示例**（截断）

```json
{
  "total": 0,
  "error_code": 0,
  "data": [],
  "extra": {
    "page_total": 0
  },
  "status": 1,
  "errmsg": ""
}
```

### /artist/audios

歌手音频列表

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | string | 是 | - | 歌手ID |
| sort | string | 否 | 2 | 排序 hot=1 其他=2 |
| pagesize | number | 否 | 30 | 每页数量 |
| page | number | 否 | 1 | 页码 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
total: number
error_code: number
data: array  // array<object>，元素字段见下
  └ [].video_track: number
  └ [].filesize_flac: number
  └ [].old_hide_flac: number
  └ [].playcount: number
  └ [].status: number
  └ [].video_file_head: number
  └ [].filesize_320: number
  └ [].rp_id: number
  └ [].bitrate_flac: number
  └ [].old_hide: number
  └ [].trans_param: object
  └ [].pay_type_128: number
  └ [].remarks: array
  └ [].all_privs_super: object
  └ [].video_hash: string
  └ [].is_original: number
  └ [].version: number
  └ [].topic_url_high: string
  └ [].identity: number
  └ [].album_id: number
  └ [].old_cpy_320: number
  └ [].privilege_high: number
  └ [].pay_type: number
  └ [].all_privs_128: object
  └ [].topic_url_flac: string
  └ [].rp_publish: number
  └ [].pkg_price_super: number
  └ [].pkg_price_high: number
  └ [].pkg_price_flac: number
  └ [].video_filesize: number
  └ [].price_high: number
  └ [].tmp_privilege: number
  └ [].pkg_price: number
  └ [].status_320: number
  └ [].album_audio_id: number
  └ [].pay_type_super: number
  └ [].old_hide_320: number
  └ [].pkg_price_320: number
  └ [].topic_remark: string
  └ [].pay_type_high: number
  └ [].privilege_128: number
  └ [].timelength_320: number
  └ [].price_flac: number
  └ [].fail_process_128: number
  └ [].audio_name: string
  └ [].timelength_flac: number
  └ [].price_320: number
  └ [].pay_type_flac: number
  └ [].topic_url: string
  └ [].cid: number
  └ [].audio_file_head: number
  └ [].filesize_super: number
  └ [].topic_url_super: string
  └ [].old_cpy_super: number
  └ [].all_privs_high: object
  └ [].all_privs: object
  └ [].topic_url_320: string
  └ [].mixsong_type: number
  └ [].rp_type_320: string
  └ [].hash: string
  └ [].author_name: string
  └ [].all_privs_flac: object
  └ [].fail_process_high: number
  └ [].status_flac: number
  └ [].audio_id: number
  └ [].status_128: number
  └ [].price_128: number
  └ [].rp_type_flac: string
  └ [].pay_type_320: number
  └ [].all_privs_320: object
  └ [].timelength_high: number
  └ [].filesize_128: number
  └ [].rp_type_super: string
  └ [].album_name: string
  └ [].rp_type_high: string
  └ [].topic_url_128: string
  └ [].rp_type_128: string
  └ [].songid: number
  └ [].rp_type: string
  └ [].fail_process_super: number
  └ [].extname_super: string
  └ [].status_high: number
  └ [].publish_date: string
  └ [].hash_320: string
  └ [].bitrate_super: number
  └ [].hash_high: string
  └ [].extname: string
  └ [].fail_process_320: number
  └ [].fail_process: number
  └ [].old_cpy_high: number
  └ [].timelength_super: number
  └ [].old_cpy_flac: number
  └ [].level: number
  └ [].old_hide_super: number
  └ [].privilege_flac: number
  └ [].old_cpy: number
  └ [].status_super: number
  └ [].old_cpy_128: number
  └ [].old_hide_128: number
  └ [].video_timelength: number
  └ [].bitrate: number
  └ [].cd_url: string
  └ [].privilege_320: number
  └ [].pkg_price_128: number
  └ [].price: number
  └ [].filesize_high: number
  └ [].filesize: number
  └ [].timelength_128: number
  └ [].price_super: number
  └ [].has_obbligato: number
  └ [].hash_128: string
  └ [].album_audio_remark: string
  └ [].bitrate_high: number
  └ [].fail_process_flac: number
  └ [].hash_flac: string
  └ [].old_hide_high: number
  └ [].hash_super: string
  └ [].timelength: number
  └ [].privilege: number
  └ [].privilege_super: number
  └ [].video_id: number
extra: object
  └ page_total: number
  └ group: number
errcode: number
status: number
errmsg: string
```

**返回示例**（截断）

```json
{
  "total": 1820,
  "error_code": 0,
  "data": [
    {
      "video_track": 3,
      "filesize_flac": 24636320,
      "old_hide_flac": 1,
      "playcount": 0,
      "status": 0,
      "video_file_head": 1,
      "filesize_320": 8096075,
      "rp_id": 1105012556,
      "bitrate_flac": 973,
      "old_hide": 1,
      "trans_param": {
        "cpy_grade": 1,
        "classmap": {
          "attr0": 234881032
        },
        "language": "国语",
        "cpy_attr0": 41950336,
        "musicpack_advance": 1,
        "display": 0,
        "display_rate": 0,
        "ogg_320_hash": "8FC8D6793BD7B789B0C63D76B9E11CD0",
        "provider": 2,
        "cpy_map": "2",
        "union_cover": "http://imge.kugou.com/stdmusic/{size}/20260319/20260319101420611881.jpg",
        "ogg_320_filesize": 9098944,
        "qualitymap": {
          "bits": "8fffc0db411fefebf12c074",
          "attr0": 1058193524,
          "attr1": 1747189245
        },
        "is_original": 1,
        "ogg_128_filesize": 2564438,
        "cid": 579895220,
        "ogg_128_hash": "8DC5A2250F2C280C3079817860EA65C2",
        "pay_block_tpl": 1,
        "ipmap": {
          "attr0": 138546253824
        },
        "hash_offset": {
          "clip_hash": "69F81D9ED2E106B86F1BB827FBADB3DE",
          "start_byte": 0,
          "end_ms": 60000,
          "end_byte": 960129,
          "file_type": 0,
          "start_ms": 0,
          "offset_hash": "2938B1873542BB0BB1CCC2B3AF6B9AA8"
        },
        "appid_block": "3124",
        "hash_multitrack": "F952A51B2ADEED6C0CC475A8695CC891",
        "cpy_level": 1
      },
 
  ...(已截断，完整数据见 responses/)
```

### /artist/detail

歌手详情

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | string | 是 | - | 歌手ID |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
msg: string
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "data": {},
  "msg": "",
  "status": 1,
  "error_code": 0
}
```

### /artist/follow

关注歌手

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | number | 是 | 0 | 歌手ID |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /artist/follow/newsongs

关注歌手的新歌

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| last_album_id | number | 否 | 0 | 上次专辑ID |
| pagesize | number | 否 | 30 | 每页数量 |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /artist/honour

歌手荣誉/奖项

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | string | 是 | - | 歌手ID |
| pagesize | number | 否 | 30 | 每页数量 |
| page | number | 否 | 1 | 页码 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
error_msg: string
error_code: number
data: object
  └ info_list: array
status: number
serice_ts: number
```

**返回示例**（截断）

```json
{
  "error_msg": "success",
  "error_code": 0,
  "data": {
    "info_list": []
  },
  "status": 1,
  "serice_ts": 1788516836
}
```

### /artist/lists

歌手分类列表

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| musician | number | 否 | 0 | 是否音乐人 |
| sextypes | number | 否 | 0 | 性别类型 |
| type | number | 否 | 0 | 类型 |
| hotsize | number | 否 | 30 | 热门数量 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ timestamp: number
  └ info: array
  └ enu_list: object
errcode: number
status: number
error: string
```

**返回示例**（截断）

```json
{
  "data": {
    "timestamp": 1788516837,
    "info": [
      {
        "singer": [
          {
            "albumcount": 0,
            "dycover": {
              "first_frame_image": "http://singerimg.kugou.com/uploadpic/softhead/{size}/20260807/20260807113446764474.jpg"
            },
            "descibe": "3.6万条问答",
            "offline_url": "https://miniapp.kugou.com/node/v2?type=1&id=120&path=%2Findex.html%23%2Fsinger2%2F3520%2F25612056",
            "heatoffset": 0,
            "singerid": 3520,
            "songcount": 0,
            "fanscount": 25612159,
            "singername": "周杰伦",
            "is_settled": 0,
            "intro": "",
            "mvcount": 0,
            "url": "https://h5.kugou.com/apps/singer-qa-v2/#/singer2/3520/25612056",
            "sortoffset": -33,
            "heat": 45425,
            "imgurl": "http://singerimg.kugou.com/uploadpic/softhead/{size}/20260324/20260324203006202058.jpg"
          },
          {
            "albumcount": 0,
            "dycover": {
              "first_frame_image": ""
            },
            "descibe": "9万条问答",
            "offline_url": "https://miniapp.kugou.com/node/v2?type=1&id=120&path=%2Findex.html%23%2Fsinger2%2F3060%2F19974906",
            "heatoffset": 0,
            "singerid": 3060,
            "songcount": 0,
            "fanscount": 19974936,
            "singername": "薛之谦",
            "is_settled": 1,
            "intro": "",
            "mvcount": 0,
            "url": "https://h5.kugou.com/apps/singer-qa-v2/#/singer2/3060/19974906",
            "sortoffset": -39,
            "hea
  ...(已截断，完整数据见 responses/)
```

### /artist/unfollow

取消关注歌手

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | string | 是 | - | 歌手ID |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /artist/videos

歌手视频列表

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | string | 是 | - | 歌手ID |
| tag | string | 否 | all | 标签 official/live/fan/artist |
| pagesize | number | 否 | 30 | 每页数量 |
| page | number | 否 | 1 | 页码 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
status: number
error_code: number
errcode: number
errmsg: string
total: number
extra: object
  └ page_total: number
data: array  // array<object>，元素字段见下
  └ [].audio_bitrate: number
  └ [].publish_date: string
  └ [].mkv_sd_hash: string
  └ [].track: number
  └ [].album_audio_id: number
  └ [].mkv_qhd_hash: string
  └ [].hdpic: string
  └ [].history_heat: number
  └ [].heat: number
  └ [].user_name: string
  └ [].audio_filesize: number
  └ [].cover: string
  └ [].user_id: string
  └ [].topic: string
  └ [].remark: string
  └ [].extern: string
  └ [].timelength: number
  └ [].audio_hash: string
  └ [].pic: string
  └ [].mkv_sd_filesize: string
  └ [].video_name: string
  └ [].video_id: number
  └ [].intro: string
  └ [].audio_timelength: string
  └ [].author_name: string
  └ [].h264: object
  └ [].type: number
  └ [].is_short: number
```

**返回示例**（截断）

```json
{
  "status": 1,
  "error_code": 0,
  "errcode": 0,
  "errmsg": "",
  "total": 2823,
  "extra": {
    "page_total": 2823
  },
  "data": [
    {
      "audio_bitrate": 128,
      "publish_date": "2026-06-27 21:20:42",
      "mkv_sd_hash": "D6944CFF89D1EF774F65B1C6AB173E93",
      "track": 3,
      "album_audio_id": 64574642,
      "mkv_qhd_hash": "7C2B55C5CD04B849257CEDFCA9E2038C",
      "hdpic": "http://imge.kugou.com/mvhdpic/{size}/20260628/20260628233021672947.jpg",
      "history_heat": 4522,
      "heat": 32,
      "user_name": "音浪乱乱撞",
      "audio_filesize": 3964978,
      "cover": "http://imge.kugou.com/mvpic/d6/94/d6944cff89d1ef774f65b1c6ab173e93.jpg",
      "user_id": "2478730337",
      "topic": "嘉年华世界巡回演唱会",
      "remark": "",
      "extern": "",
      "timelength": 90233,
      "audio_hash": "8AC4F90B1E3DF716D216D74223016E17",
      "pic": "20260628233021672947.jpg",
      "mkv_sd_filesize": "7485118",
      "video_name": "《千山万水》饭拍 周杰伦 #2026龙拳·嘉年华世界巡回演唱会北京站",
      "video_id": 16191180,
      "intro": "",
      "audio_timelength": "247000",
      "author_name": "音浪乱乱撞",
      "h264": {
        "ld_hash": "16C8A2C0787843C5E05F678E04AFB0CA",
        "ld_filesize": 4179259,
        "ld_bitrate": 267834,
        "sd_hash": "D6944CFF89D1EF774F65B1C6AB173E93",
        "sd_filesize": 7485118,
        "sd_bitrate": 529027,
        "qhd_hash": "7C2B55C5CD04B849257CEDFCA9E2038C",
        "qhd_filesize": 12003527,
        "qhd_bitrate": 929772,
        "hd_hash": "",
        "hd_filesize": 0,
        "hd_bitrate": 0,
        "fhd_hash": "8EFBDF9CE8A34B81B755C7565CECDE22",
  ...(已截断，完整数据见 responses/)
```

### /singer/list

歌手列表（按分类）

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| musician | number | 否 | 0 | 是否音乐人 |
| sextypes | number | 否 | 0 | 性别类型 |
| type | number | 否 | 0 | 类型 |
| hotsize | number | 否 | 30 | 热门数量 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ timestamp: number
  └ info: array
  └ enu_list: object
errcode: number
status: number
error: string
```

**返回示例**（截断）

```json
{
  "data": {
    "timestamp": 1788516866,
    "info": [
      {
        "singer": [
          {
            "albumcount": 0,
            "dycover": {
              "first_frame_image": "http://singerimg.kugou.com/uploadpic/softhead/{size}/20260807/20260807113446764474.jpg"
            },
            "descibe": "3.6万条问答",
            "offline_url": "https://miniapp.kugou.com/node/v2?type=1&id=120&path=%2Findex.html%23%2Fsinger2%2F3520%2F25612056",
            "heatoffset": 0,
            "singerid": 3520,
            "songcount": 0,
            "fanscount": 25612160,
            "singername": "周杰伦",
            "is_settled": 0,
            "intro": "",
            "mvcount": 0,
            "url": "https://h5.kugou.com/apps/singer-qa-v2/#/singer2/3520/25612056",
            "sortoffset": -33,
            "heat": 45425,
            "imgurl": "http://singerimg.kugou.com/uploadpic/softhead/{size}/20260324/20260324203006202058.jpg"
          },
          {
            "albumcount": 0,
            "dycover": {
              "first_frame_image": ""
            },
            "descibe": "9万条问答",
            "offline_url": "https://miniapp.kugou.com/node/v2?type=1&id=120&path=%2Findex.html%23%2Fsinger2%2F3060%2F19974906",
            "heatoffset": 0,
            "singerid": 3060,
            "songcount": 0,
            "fanscount": 19974937,
            "singername": "薛之谦",
            "is_settled": 1,
            "intro": "",
            "mvcount": 0,
            "url": "https://h5.kugou.com/apps/singer-qa-v2/#/singer2/3060/19974906",
            "sortoffset": -39,
            "hea
  ...(已截断，完整数据见 responses/)
```

## 歌单（sheet/playlist）

### /playlist/add

新建歌单

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| name | string | 是 | - | 歌单名称 |
| type | number | 否 | 0 | 类型 |
| source | number | 否 | 1 | 来源 |
| list_create_userid | string | 是 | - | 原歌单创建者userid |
| list_create_listid | string | 是 | - | 原歌单ID |
| list_create_gid | string | 是 | - | 原歌单gid |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /playlist/del

删除歌单

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| listid | number | 是 | - | 歌单ID |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /playlist/detail

歌单详情

> `ids` 支持逗号分隔多个 `global_collection_id`。

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| ids | string | 是 | - | 歌单ID(逗号分隔) |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: array  // array<object>，元素字段见下
  └ [].tags: string
  └ [].status: number
  └ [].create_user_pic: string
  └ [].is_pri: number
  └ [].pub_new: number
  └ [].is_drop: number
  └ [].list_create_userid: number
  └ [].is_publish: number
  └ [].musiclib_tags: array
  └ [].pub_type: number
  └ [].is_featured: number
  └ [].publish_date: string
  └ [].collect_total: number
  └ [].list_ver: number
  └ [].intro: string
  └ [].type: number
  └ [].list_create_listid: number
  └ [].specialid: number
  └ [].radio_id: number
  └ [].source: number
  └ [].trans_param: object
  └ [].listid: number
  └ [].is_def: number
  └ [].parent_global_collection_id: string
  └ [].sound_quality: string
  └ [].per_count: number
  └ [].plist: array
  └ [].kq_talent: number
  └ [].create_time: number
  └ [].is_per: number
  └ [].is_edit: number
  └ [].update_time: number
  └ [].code: number
  └ [].count: number
  └ [].sort: number
  └ [].is_mine: number
  └ [].musiclib_id: number
  └ [].per_num: number
  └ [].create_user_gender: number
  └ [].number: number
  └ [].pic: string
  └ [].list_create_username: string
  └ [].name: string
  └ [].is_custom_pic: number
  └ [].global_collection_id: string
  └ [].heat: number
  └ [].list_create_gid: string
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "data": [
    {
      "tags": "流行,日语,游戏,ACG",
      "status": 1,
      "create_user_pic": "http://imge.kugou.com/kugouicon/165/20161103/20161103181052105.jpg",
      "is_pri": 0,
      "pub_new": 0,
      "is_drop": 0,
      "list_create_userid": 509005933,
      "is_publish": 1,
      "musiclib_tags": [
        {
          "tag_id": 9,
          "parent_id": 8,
          "tag_name": "流行"
        },
        {
          "tag_id": 21,
          "parent_id": 2,
          "tag_name": "日语"
        },
        {
          "tag_id": 74,
          "parent_id": 1,
          "tag_name": "游戏"
        },
        {
          "tag_id": 77,
          "parent_id": 1,
          "tag_name": "ACG"
        }
      ],
      "pub_type": 1,
      "is_featured": 0,
      "publish_date": "2020-09-01",
      "collect_total": 0,
      "list_ver": 2,
      "intro": "天空燃尽如灰，繁星烧毁似尘，那些音乐所带来的力量，化作为光，将引领着我们前行！",
      "type": 0,
      "list_create_listid": 33,
      "specialid": 2927982,
      "radio_id": 0,
      "source": 1,
      "trans_param": {
        "iden": 0
      },
      "listid": 33,
      "is_def": 0,
      "parent_global_collection_id": "collection_1_509005933_2927982_0",
      "sound_quality": "",
      "per_count": 0,
      "plist": [],
      "kq_talent": 0,
      "create_time": 1598926530,
      "is_per": 0,
      "is_edit": 1,
      "update_time": 1735836959,
      "code": 1,
      "count": 232,
      "sort": 0,
      "is_mine": 0,
      "musiclib_id": 0,
      "per_num": 0,
      "create_user_gender": 0,
      "number": 0,
      "pic": "http://imge.kugou.com/soft/collection/{size}/201
  ...(已截断，完整数据见 responses/)
```

### /playlist/effect

歌单效果（标签氛围）

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ has_next: number
  └ total: number
  └ info: array
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "data": {
    "has_next": 1,
    "total": 2673,
    "info": [
      {
        "list_create_listid": 9,
        "deleted_or_404": 0,
        "attribute_bit": 0,
        "create_user_pic": "http://imge.kugou.com/kugouicon/165/20231031/20231031140018557326.jpg",
        "is_pri": 0,
        "special_tag": 0,
        "source": 1,
        "intro": "我很庆幸曾经能够走到你的身边，和你有过那么多美好的回忆；但也很遗憾，终究没能走进你的心里，成为你生命中那个最重要的人。",
        "list_create_userid": 2159648969,
        "update_time": 1786693251,
        "list_ver": 282,
        "musiclib_tags": [
          {
            "tag_id": 9,
            "tag_name": "流行",
            "parent_id": 8
          },
          {
            "tag_id": 35,
            "tag_name": "网络",
            "parent_id": 1
          },
          {
            "tag_id": 571,
            "tag_name": "00后",
            "parent_id": 7
          }
        ],
        "list_create_gid": "collection_3_2159648969_9_0",
        "trans_param": {
          "special_tag": 0,
          "iden": 0,
          "trans_flag": 1
        },
        "name": "深夜emo歌曲：很遗憾没能走进你的心里",
        "auditinfo": {
          "review_status": 1,
          "status": 1,
          "is_drop": 0
        },
        "plist": {},
        "count": 156,
        "pic": "http://c1.kgimg.com/custom/{size}/20231201/20231201113708239990.jpg",
        "list_create_username": "鹿明明",
        "pub_type": 2,
        "sound": {
          "id": "A1",
          "type": 1,
          "args": ""
        },
        "global_collection_id": "collection_3_2159648969_9_0",
        "heat": 33405841,
        "type": 0
      },
     
  ...(已截断，完整数据见 responses/)
```

### /playlist/similar

相似歌单推荐

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| ids | string | 是 | - | 歌单ID(逗号分隔) |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
error_code: number
data: array  // array<object>，元素字段见下
  └ [].collection_list: array
  └ [].tag_info: array
status: number
```

**返回示例**（截断）

```json
{
  "error_code": 0,
  "data": [
    {
      "collection_list": [
        {
          "flexible_cover": "http://c1.kgimg.com/custom/{size}/20250425/20250425204651940074.jpg",
          "user_name": "时光是首无声的歌",
          "is_recommend": "0",
          "user_is_verified": "0",
          "collection_name": "Eep专属跑步歌单｜前方高能预警，解锁超燃运动时刻",
          "heat": "9510",
          "sync": "0",
          "ugc_talent_info": {
            "review_status": "1"
          },
          "is_publish": "1",
          "special_tag": "0",
          "songs": [
            {
              "sort": 0,
              "level": 1,
              "hash": "56106A483D85685A8CBB32EEC1B0232A",
              "cover": "http://imge.kugou.com/stdmusic/{size}/20220423/20220423084810688707.jpg",
              "trans_param": {
                "ogg_128_hash": "C3B3FC0E8EDC9F69590F2BB31638C87B",
                "union_cover": "http://imge.kugou.com/stdmusic/{size}/20220423/20220423084810688707.jpg",
                "language": "英语",
                "cpy_attr0": 50339840,
                "musicpack_advance": 0,
                "ogg_128_filesize": 1593259,
                "display_rate": 0,
                "cpy_map": "2",
                "qualitymap": {
                  "bits": "1b4100000773fc034",
                  "attr0": 2000666676,
                  "attr1": 1746927616
                },
                "is_original": 1,
                "classmap": {
                  "attr0": 234881032
                },
                "cid": 206000375,
                "cpy_grade": 5,
                "hash_multitrack": "33E32B416159
  ...(已截断，完整数据见 responses/)
```

### /playlist/tags

歌单标签

**请求参数**：无

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: array  // array<object>，元素字段见下
  └ [].parent_id: string
  └ [].sort: string
  └ [].tag_id: string
  └ [].tag_name: string
  └ [].son: array
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "data": [
    {
      "parent_id": "0",
      "sort": "1",
      "tag_id": "5",
      "tag_name": "场景",
      "son": [
        {
          "parent_id": "5",
          "tag_id": "587",
          "tag_name": "学习",
          "sort": "1"
        },
        {
          "parent_id": "5",
          "tag_id": "660",
          "tag_name": "工作",
          "sort": "2"
        },
        {
          "parent_id": "5",
          "tag_id": "1104",
          "tag_name": "通勤",
          "sort": "3"
        },
        {
          "parent_id": "5",
          "tag_id": "69",
          "tag_name": "运动",
          "sort": "4"
        },
        {
          "parent_id": "5",
          "tag_id": "67",
          "tag_name": "校园",
          "sort": "5"
        },
        {
          "parent_id": "5",
          "tag_id": "585",
          "tag_name": "旅途",
          "sort": "6"
        },
        {
          "parent_id": "5",
          "tag_id": "61",
          "tag_name": "咖啡厅",
          "sort": "7"
        },
        {
          "parent_id": "5",
          "tag_id": "581",
          "tag_name": "店铺",
          "sort": "8"
        },
        {
          "parent_id": "5",
          "tag_id": "668",
          "tag_name": "清晨",
          "sort": "9"
        },
        {
          "parent_id": "5",
          "tag_id": "670",
          "tag_name": "下午茶",
          "sort": "10"
        },
        {
          "parent_id": "5",
          "tag_id": "1105",
          "tag_name": "夜晚",
          "sort": "11"
        },
        {
          "parent_id": "5",
          "tag_id": "583",
          "tag_name": "
  ...(已截断，完整数据见 responses/)
```

### /playlist/track/all

歌单内全部曲目

> `id` 需为歌单的 `global_collection_id`（可从 /top/playlist 返回的 `special_list[].global_collection_id` 获取）。

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | string | 是 | - | 歌单ID |
| pagesize | number | 否 | 30 | 每页数量 |
| page | number | 否 | 1 | 页码 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
error_code: number
errmsg: string
data: object
  └ begin_idx: number
  └ pagesize: number
  └ count: number
  └ userid: number
  └ list_info: object
  └ songs: array
  └ popularization: object
status: number
```

**返回示例**（截断）

```json
{
  "error_code": 0,
  "errmsg": "",
  "data": {
    "begin_idx": 0,
    "pagesize": 5,
    "count": 232,
    "userid": 509005933,
    "list_info": {
      "tags": "流行,日语,游戏,ACG",
      "status": 1,
      "create_user_pic": "http://imge.kugou.com/kugouicon/165/20161103/20161103181052105.jpg",
      "is_pri": 0,
      "pub_new": 0,
      "is_drop": 0,
      "list_create_userid": 509005933,
      "is_publish": 1,
      "musiclib_tags": [
        {
          "tag_id": 9,
          "parent_id": 8,
          "tag_name": "流行"
        },
        {
          "tag_id": 21,
          "parent_id": 2,
          "tag_name": "日语"
        },
        {
          "tag_id": 74,
          "parent_id": 1,
          "tag_name": "游戏"
        },
        {
          "tag_id": 77,
          "parent_id": 1,
          "tag_name": "ACG"
        }
      ],
      "pub_type": 1,
      "is_featured": 0,
      "publish_date": "2020-09-01",
      "collect_total": 0,
      "list_ver": 2,
      "intro": "天空燃尽如灰，繁星烧毁似尘，那些音乐所带来的力量，化作为光，将引领着我们前行！",
      "type": 0,
      "list_create_listid": 33,
      "specialid": 2927982,
      "radio_id": 0,
      "source": 1,
      "trans_param": {
        "iden": 0
      },
      "code": 1,
      "is_def": 0,
      "parent_global_collection_id": "collection_1_509005933_2927982_0",
      "sound_quality": "",
      "per_count": 0,
      "plist": [],
      "create_time": 1598926530,
      "is_per": 0,
      "is_edit": 1,
      "update_time": 1735836959,
      "per_num": 0,
      "count": 232,
      "sort": 0,
      "listid": 33,
      "musiclib_id": 0,
      "kq_talent": 0,
  
  ...(已截断，完整数据见 responses/)
```

### /playlist/track/all/new

歌单内全部曲目（新版）

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| listid | string | 是 | - | 歌单ID |
| pagesize | number | 否 | 30 | 每页数量 |
| page | number | 否 | 1 | 页码 |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /playlist/tracks/add

歌单添加歌曲

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| listid | string | 是 | - | 歌单ID |
| data | string | 是 | - | 歌曲数据 name|hash|album_id|mixsongid,逗号分隔 |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /playlist/tracks/del

歌单移除歌曲

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| listid | string | 是 | - | 歌单ID |
| fileids | string | 是 | - | 文件ID(逗号分隔) |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /sheet/collection

歌单合集

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| position | number | 否 | 2 | 位置 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ good_selection: object
errcode: number
status: number
error: string
```

**返回示例**（截断）

```json
{
  "data": {
    "good_selection": {
      "list": [
        {
          "tag_list": [
            "推荐",
            "钢琴"
          ],
          "title": "宝藏推荐 | 那些有意思的独立音乐人的谱单",
          "collection_type": 1,
          "view_num": 486900,
          "collection_id": "59",
          "song_list": [
            {
              "song_name": "夜空中最亮的星",
              "album_audio_id": 32054655,
              "audio_id": 104410
            },
            {
              "song_name": "南山南",
              "album_audio_id": 32145284,
              "audio_id": 4257452
            },
            {
              "song_name": "骄傲的少年",
              "album_audio_id": 36993945,
              "audio_id": 21087182
            }
          ],
          "info": "",
          "icon": "http://mobileservice.bssdl.kugou.com/3905240f7ab9003c2a5cebb6c91a1207.jpg"
        },
        {
          "tag_list": [
            "推荐",
            "吉他"
          ],
          "title": "吉他谱 | 华语流行热歌精选",
          "collection_type": 1,
          "view_num": 357641,
          "collection_id": "69",
          "song_list": [
            {
              "song_name": "最佳损友 (To My Friend Special Mix)",
              "album_audio_id": 28139637,
              "audio_id": 169738
            },
            {
              "song_name": "夜空中最亮的星",
              "album_audio_id": 32054655,
              "audio_id": 104410
            },
            {
              "song_name": "你就不要想起我",
              "album_audio_id": 32089443,
              "audio_id": 985220
            }
          ],
          "info": "",
          "icon
  ...(已截断，完整数据见 responses/)
```

### /sheet/collection/detail

歌单合集详情

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| collection_id | string | 是 | - | 乐谱合集ID |
| page | number | 否 | 1 | 页码 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ img_url: string
  └ total: number
  └ name: string
  └ opern_list: array
  └ author_avatar: string
  └ author: string
  └ info: string
  └ collection_type: number
  └ view_num: number
  └ userid: number
  └ song_list: array
  └ tag_list: array
  └ opern_type: number
errcode: number
status: number
error: string
```

**返回示例**（截断）

```json
{
  "data": {
    "img_url": "http://imge.kugou.com/quickentry/20211012/20211012172540588497.jpg",
    "total": 9,
    "name": "经典钢琴曲",
    "opern_list": [],
    "author_avatar": "http://imge.kugou.com/kugouicon/165/20160113/20160113180717663307.jpg",
    "author": "第三方",
    "info": "",
    "collection_type": 1,
    "view_num": 55282,
    "userid": 520032960,
    "song_list": [
      {
        "is_publish": 1,
        "pic": "http://imge.kugou.com/stdmusic/{size}/20241118/20241118143502768505.jpg",
        "hash": "3E6D534A58AED5BCDAB1F38C1A0B4496",
        "audio_id": 312591,
        "album_audio_id": 32080691,
        "author": "张信哲",
        "song_name": "爱就一个字",
        "view_num": 6448
      },
      {
        "is_publish": 1,
        "pic": "http://imge.kugou.com/stdmusic/{size}/20250125/20250125121625606416.jpg",
        "hash": "BA9D67DAB59FA6DCE243F4B6B70ACA95",
        "audio_id": 246787,
        "album_audio_id": 32105156,
        "author": "林忆莲",
        "song_name": "至少还有你",
        "view_num": 6663
      },
      {
        "is_publish": 1,
        "pic": "http://imge.kugou.com/stdmusic/{size}/20250516/20250516175502439257.jpg",
        "hash": "407277CFBF937723C231EB2516F3C8C6",
        "audio_id": 57540,
        "album_audio_id": 32137411,
        "author": "李健",
        "song_name": "贝加尔湖畔",
        "view_num": 21799
      },
      {
        "is_publish": 1,
        "pic": "http://imge.kugou.com/stdmusic/{size}/20250125/20250125121726584286.jpg",
        "hash": "3B343C47D4605A53A605BDA51CE1666D",
        "audio_id": 345056,
        "album_audio_id": 321397
  ...(已截断，完整数据见 responses/)
```

### /sheet/detail

歌单详情

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | string | 是 | - | 乐谱ID |
| source | string | 否 | - | 来源 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: null
errcode: number
status: number
error: string
```

**返回示例**（截断）

```json
{
  "data": null,
  "errcode": 0,
  "status": 1,
  "error": ""
}
```

### /sheet/hot

热门歌单

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| opern_type | number | 否 | 1 | 乐谱类型 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ list: array
errcode: number
status: number
error: string
```

**返回示例**（截断）

```json
{
  "data": {
    "list": [
      {
        "opern_author": "温奕心",
        "hash": "4F18553F2AF06E355AD75A5EE6173456",
        "source": 2,
        "third": {
          "id": ""
        },
        "audio_id": 92351310,
        "song_name": "一路生花",
        "opern_type": 1,
        "opern_name": "一路生花",
        "album_audio_id": 317568292,
        "score_type": 2,
        "mark": "",
        "opern_id": "1575308687008997376",
        "display_version": "version1",
        "pay_type": 1,
        "jump_type": 4,
        "opern_cover": "http://imge.kugou.com/stdmusic/{size}/20251016/20251016152611214074.jpg",
        "view_num": 373465
      },
      {
        "opern_author": "陈奕迅",
        "hash": "64B39A6828913E58E1ED1F7F6061A380",
        "source": 2,
        "third": {
          "id": ""
        },
        "audio_id": 127845904,
        "song_name": "孤勇者",
        "opern_type": 1,
        "opern_name": "孤勇者",
        "album_audio_id": 348254775,
        "score_type": 2,
        "mark": "",
        "opern_id": "1575308753463549952",
        "display_version": "version1",
        "pay_type": 1,
        "jump_type": 4,
        "opern_cover": "http://imge.kugou.com/stdmusic/{size}/20211108/20211108095904355171.jpg",
        "view_num": 227781
      },
      {
        "opern_type": 1,
        "opern_author": "周杰伦",
        "hash": "B1643F7EDAD1AE5EF306174F94C57BA6",
        "source": 1,
        "pay_type": 1,
        "opern_name": "还在流浪",
        "opern_id": "1003972908391891620",
        "view_num": 57385,
        "score_type": 2,
        "song_name": "还在流浪",
        "display_ve
  ...(已截断，完整数据见 responses/)
```

### /sheet/list

歌单列表

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| album_audio_id | string | 否 | - | 音频ID |
| opern_type | number | 否 | 0 | 乐谱类型 |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ song: object
  └ total: number
  └ list: array
errcode: number
status: number
error: string
```

**返回示例**（截断）

```json
{
  "data": {
    "song": {
      "is_publish": 1,
      "hash": "4F18553F2AF06E355AD75A5EE6173456",
      "author_name": "温奕心",
      "album_audio_id": 317568292,
      "album_id": 45305792,
      "audio_length": 256286,
      "album_cover": "http://imge.kugou.com/stdmusic/{size}/20251016/20251016152611214074.jpg",
      "audio_id": 92351310,
      "ori_audio_id": 92351310,
      "songname": "一路生花"
    },
    "total": 17,
    "list": [
      {
        "opern_tune": "",
        "id": "317568292_5_4",
        "version_info": "智能尤克里里谱",
        "ai_origin_tune": "",
        "third": {
          "qmusic_songmid": "004AxmlM3YihD2",
          "preview_url": "https://y.qq.com/m/client/guitar_community/smart_guitar.html?songMid=004AxmlM3YihD2&noLoginMode=1",
          "sheet_data_type": "1"
        },
        "avg_score": 0,
        "jump_type": 5,
        "userid": 520033058,
        "opern_type": 4,
        "created_at": "",
        "collection_source": 16,
        "pics": [],
        "is_blocked": 0,
        "pay_type": 1,
        "score_type": 3,
        "g_stats": {
          "display_count": 0
        },
        "released_at": "",
        "report_opern_type": 9,
        "g_user": {
          "userid": 520033058,
          "nickname": "曲谱助手",
          "avatar": "http://imge.kugou.com/kugouicon/165/20230426/20230426104456726952.jpg"
        },
        "source": 5,
        "is_released": 1
      },
      {
        "opern_tune": "",
        "id": "317568292_5_1",
        "version_info": "智能钢琴谱",
        "ai_origin_tune": "",
        "third": {
          "qmusic_songmid": "004Ax
  ...(已截断，完整数据见 responses/)
```

## 排行榜

### /rank/audio

榜内音频

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| rankid | string | 是 | - | 排行榜ID |
| rank_cid | number | 否 | 0 | 排行榜分类ID |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
total: number
error_code: number
data: object
  └ total: number
  └ songlist: array
extra: object
  └ resp: object
status: number
errmsg: string
```

**返回示例**（截断）

```json
{
  "total": 500,
  "error_code": 0,
  "data": {
    "total": 500,
    "songlist": [
      {
        "mvdata": [
          {
            "typ": 0,
            "trk": "3",
            "hash": "0D94287D8E062EAB8424F4E239983D1A",
            "id": "15758336"
          }
        ],
        "authors": [
          {
            "sizable_avatar": "http://singerimg.kugou.com/uploadpic/softhead/{size}/20260414/20260414101321856856.jpg",
            "is_publish": 1,
            "author_id": 21321931,
            "author_name": "阿图表妹"
          }
        ],
        "author_name": "阿图表妹",
        "copyright": {
          "old_hide": 1
        },
        "audio_id": 571484095,
        "video_info": {
          "video_id": 0,
          "video_track": 0,
          "video_timelength": 0,
          "video_filesize": 0,
          "video_hash": ""
        },
        "musical": {},
        "remarks": [
          {
            "rel_album_audio_id": 0,
            "remark": "谈柒柒",
            "type": 5
          }
        ],
        "trans_param": {
          "cpy_grade": 5,
          "classmap": {
            "attr0": 234881032
          },
          "language": "国语",
          "cpy_attr0": 58735744,
          "musicpack_advance": 1,
          "display": 0,
          "display_rate": 0,
          "ogg_320_hash": "5B663FCD882D5A913BBE6ACA478A7FBF",
          "provider": 5,
          "cpy_map": "2",
          "union_cover": "http://imge.kugou.com/stdmusic/{size}/20260604/20260604164021847508.jpg",
          "ogg_320_filesize": 9460902,
          "qualitymap": {
            "bits": "8fffe0db411fefe
  ...(已截断，完整数据见 responses/)
```

### /rank/info

榜单信息

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| rankid | string | 是 | - | 排行榜ID |
| rank_cid | number | 否 | 0 | 排行榜分类ID |
| album_img | number | 否 | 1 | 是否含专辑图 |
| zone | string | 否 | - | 地区 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ custom_logo: string
  └ count_down: number
  └ rankname: string
  └ use_custom_logo: number
  └ banner_9: string
  └ rankid: number
  └ custom_logo_4: string
  └ table_plaque: string
  └ tip_jump_url: string
  └ is_publish: number
  └ img_9: string
  └ ranktype: number
  └ classify: number
  └ tab_rank: array
  └ intro: string
  └ timestamp: number
  └ rank_cid: number
  └ share_bg: string
  └ custom_logo_2: string
  └ jump_url: string
  └ use_custom_logo_2: number
  └ bannerurl: string
  └ extra: object
  └ songinfo: array
  └ update_video_url: string
  └ related_rank: array
  └ is_latest_realtime: number
  └ video_ending: string
  └ custom_type: number
  └ update_frequency_type: number
  └ zone: string
  └ h5_jump_url: string
  └ banner7url: string
  └ album_img_9: string
  └ share_logo: string
  └ is_timing: number
  └ img_logo: string
  └ is_limit_view_vol: number
  └ new_cycle: number
  └ imgurl: string
errcode: number
status: number
error: string
```

**返回示例**（截断）

```json
{
  "data": {
    "custom_logo": "http://imge.kugou.com/mcommon/{size}/20240109/20240109203134825852.png",
    "count_down": 1800,
    "rankname": "TOP500",
    "use_custom_logo": 0,
    "banner_9": "http://imge.kugou.com/mcommon/{size}/20220112/20220112175202472916.png",
    "rankid": 8888,
    "custom_logo_4": "https://mobileservicebssdl.kugou.com/6773ac3aabae60f85e9149d1956d94a6.png",
    "table_plaque": "http://imge.kugou.com/mcommon/{size}/20240311/20240311161214589894.png",
    "tip_jump_url": "",
    "is_publish": 1,
    "img_9": "http://imge.kugou.com/mcommon/{size}/20241219/20241219164221229666.png",
    "ranktype": 2,
    "classify": 1,
    "tab_rank": [],
    "intro": "数据来源：全曲库歌曲\r\n排序方式：按歌曲喜爱用户数的总量排序\r\n更新频率：每天",
    "timestamp": 1788516864,
    "rank_cid": 126025,
    "share_bg": "http://imge.kugou.com/mcommon/{size}/20240505/20240505213952649412.png",
    "custom_logo_2": "http://imge.kugou.com/mcommon/{size}/20241011/20241011161814657035.png",
    "jump_url": "",
    "use_custom_logo_2": 1,
    "bannerurl": "http://imge.kugou.com/mcommonbanner/{size}/20181019/20181019122517263545.jpg",
    "extra": {
      "resp": {
        "scheduled_release_conf": {
          "scheduled_release_time": "10:00:00",
          "latest_rank_cid": 126025,
          "latest_rank_cid_publish_date": "2026-09-04 08:30:00"
        },
        "five_year_total": 181,
        "new_total": 10,
        "enjoy_total": 0,
        "recent_year_total": 123,
        "follow_total": 0,
        "all_total": 500,
        "vip_total": 0,
        "rank_tag": [
          {
            "desc": "有10首上新
  ...(已截断，完整数据见 responses/)
```

### /rank/list

排行榜列表

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| withsong | number | 否 | 1 | 是否含歌曲 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ timestamp: number
  └ total: number
  └ show_line: number
  └ theme: object
  └ info: array
errcode: number
status: number
error: string
```

**返回示例**（截断）

```json
{
  "data": {
    "timestamp": 1788516864,
    "total": 57,
    "show_line": 2,
    "theme": {
      "classify_list": [],
      "bg_image": "",
      "font": {
        "nt": "",
        "st": "",
        "line": "",
        "bold_line": ""
      }
    },
    "info": [
      {
        "children": [],
        "base_img": "",
        "rankname": "TOP500",
        "new_cycle": 1800,
        "banner_9": "http://imge.kugou.com/mcommon/{size}/20220112/20220112175202472916.png",
        "album_img_9": "http://imge.kugou.com/stdmusic/{size}/20260604/20260604164021847508.jpg",
        "table_plaque": "http://imge.kugou.com/mcommon/{size}/20240311/20240311161214589894.png",
        "update_frequency_type": 1,
        "play_times": 11297979,
        "img_9": "http://imge.kugou.com/mcommon/{size}/20241219/20241219164221229666.png",
        "is_city_rank": 0,
        "classify": 1,
        "haschildren": 0,
        "songinfo": [
          {
            "album_audio_id": 879490214,
            "trans_param": {
              "cpy_grade": 5,
              "classmap": {
                "attr0": 234881032
              },
              "language": "国语",
              "cpy_attr0": 58735744,
              "musicpack_advance": 1,
              "display": 0,
              "display_rate": 0,
              "ogg_320_filesize": 9460902,
              "union_cover": "http://imge.kugou.com/stdmusic/{size}/20260604/20260604164021847508.jpg",
              "cpy_map": "2",
              "provider": 5,
              "hash_offset": {
                "clip_hash": "11588236770AEC2A2FC449F28B6447EF",
         
  ...(已截断，完整数据见 responses/)
```

### /rank/top

榜单置顶/头部内容

**请求参数**：无

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ list: array
  └ filter_ab: number
  └ theme: object
  └ activity_list: array
errcode: number
status: number
error: string
```

**返回示例**（截断）

```json
{
  "data": {
    "list": [
      {
        "album_cover_color": "#A1A7B3",
        "rankname": "90后热歌榜",
        "jump_url": "",
        "banner_9": "http://imge.kugou.com/mcommon/{size}/20220112/20220112174949199742.png",
        "album_img_9": "http://imge.kugou.com/stdmusic/{size}/20260817/20260817233114652629.jpg",
        "show_play_count": 1,
        "isvol": 1,
        "play_times": 154535,
        "img_9": "http://imge.kugou.com/mcommon/{size}/20241211/20241211185918447184.jpg",
        "custom_type": 0,
        "ranktype": 1,
        "update_frequency": "每天",
        "show_play_button": 0,
        "jump_title": "",
        "rankid": 49223,
        "banner7url": "http://imge.kugou.com/mcommon/{size}/20241211/20241211185914313564.jpg",
        "intro": "排序方式：按年龄段的喜爱热度高低排序\r\n更新频率：每天",
        "imgurl": "http://imge.kugou.com/mcommon/{size}/20241211/20241211185910253287.jpg"
      },
      {
        "album_cover_color": "#64523d",
        "rankname": "内地榜",
        "jump_url": "",
        "banner_9": "http://imge.kugou.com/mcommon/{size}/20220112/20220112175510473796.png",
        "album_img_9": "http://imge.kugou.com/stdmusic/{size}/20260831/20260831154327727967.jpg",
        "show_play_count": 1,
        "isvol": 1,
        "play_times": 99040,
        "img_9": "http://imge.kugou.com/mcommon/{size}/20241211/20241211192156257611.png",
        "custom_type": 0,
        "ranktype": 1,
        "update_frequency": "工作日",
        "show_play_button": 0,
        "jump_title": "",
        "rankid": 31308,
        "banner7url": "http://imge.kugou.com/mcommon/{size}/20241211/
  ...(已截断，完整数据见 responses/)
```

### /rank/vol

榜单期数（卷）

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| rankid | string | 是 | - | 排行榜ID |
| rank_cid | number | 否 | 0 | 排行榜分类ID |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ timestamp: number
  └ info: array
  └ vol_format: number
  └ zone: string
  └ rank_cid: number
errcode: number
status: number
error: string
```

**返回示例**（截断）

```json
{
  "data": {
    "timestamp": 1788516865,
    "info": [
      {
        "year": 2026,
        "vols": [
          {
            "volname2": "247期",
            "volid": 126025,
            "outer_text": "2026-09-04 第247期",
            "voltitle": "247期",
            "voltime": "2026-09-04 08:30:00",
            "volname": "247期",
            "special_text": ""
          },
          {
            "volname2": "246期",
            "volid": 125893,
            "outer_text": "2026-09-03 第246期",
            "voltitle": "246期",
            "voltime": "2026-09-03 08:30:00",
            "volname": "246期",
            "special_text": ""
          },
          {
            "volname2": "245期",
            "volid": 125758,
            "outer_text": "2026-09-02 第245期",
            "voltitle": "245期",
            "voltime": "2026-09-02 08:30:00",
            "volname": "245期",
            "special_text": ""
          },
          {
            "volname2": "244期",
            "volid": 125617,
            "outer_text": "2026-09-01 第244期",
            "voltitle": "244期",
            "voltime": "2026-09-01 08:30:00",
            "volname": "244期",
            "special_text": ""
          },
          {
            "volname2": "243期",
            "volid": 125482,
            "outer_text": "2026-08-31 第243期",
            "voltitle": "243期",
            "voltime": "2026-08-31 08:30:00",
            "volname": "243期",
            "special_text": ""
          },
          {
            "volname2": "242期",
            "volid": 125368,
            "outer_text": "2026-08-30 第242期",
            "vol
  ...(已截断，完整数据见 responses/)
```

### /top/album

专辑榜

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ timestamp: number
  └ chn: array
  └ eur: array
  └ jpn: array
  └ kor: array
error: string
error_code: number
status: number
```

**返回示例**（截断）

```json
{
  "data": {
    "timestamp": 1788516870,
    "chn": [
      {
        "albumid": 206223312,
        "albumname": "十个承诺九个哄 (现场版)",
        "singerid": 811890,
        "singername": "彭妃妃",
        "intro": "内地女歌手“彭妃妃”最新单曲《十个承诺九个哄》，欢迎收听！",
        "publishtime": "2026-09-04 00:00:00",
        "imgurl": "http://imge.kugou.com/stdmusic/{size}/20260903/20260903142706495461.jpg",
        "isfirst": 0,
        "songcount": 1,
        "privilege": 0,
        "trans_param": {
          "special_tag": "6668288"
        },
        "quality": 0,
        "ad_id": 0
      },
      {
        "albumid": 206223420,
        "albumname": "没有了你心好痛（广场舞版）",
        "singerid": 786970,
        "singername": "李英",
        "intro": "没有了你心好痛 没有了你爱成空\n没有了你为爱发了疯\n是你陪了我一程 是你毁了我余生\n是你让我的爱成了梦\n\n没有了你心好痛 没有了你爱成空\n没有了你爱情就随风\n是你先让我心动 是你又让我心痛\n是你让泪把我眼哭红",
        "publishtime": "2026-09-04 00:00:00",
        "imgurl": "http://imge.kugou.com/stdmusic/{size}/20260825/20260825124835655011.jpg",
        "isfirst": 0,
        "songcount": 0,
        "privilege": 0,
        "trans_param": {
          "special_tag": "4210688"
        },
        "quality": 0,
        "ad_id": 0
      },
      {
        "albumid": 206223374,
        "albumname": "情歌一曲",
        "singerid": 790163,
        "singername": "崔伟立、孙艺琪",
        "intro": "我像一颗飘在风里的蒲公英\n走过千里万里才遇见你身影\n人翻山越岭 \n梦戴月披星\n只为与你共度朝夕",
        "publishtime": "2026-09-04 00:00:00",
        "imgurl": "http://imge.kugou.com/stdmusic/{size}/20260903/20260903162709370134.jpg",
        "isfirst": 0,
        "songcount": 1,
        "privilege": 0,
        "trans_param": {
  
  ...(已截断，完整数据见 responses/)
```

### /top/ip

地域热榜

**请求参数**：无

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ timestamp: number
  └ list: array
errcode: number
status: number
error: string
```

**返回示例**（截断）

```json
{
  "data": {
    "timestamp": 1788516883,
    "list": [
      {
        "sizable_image_url": "http://imge.kugou.com/soft/collection/480/20260701/20260701191303871878.jpg_{size}x{size}.jpg",
        "id": 469313,
        "image_url": "http://imge.kugou.com/soft/collection/480/20260701/20260701191303871878.jpg",
        "title": "2026热歌合集|百听不厌的旋律里都有故事",
        "extra": {
          "play_count": 878348119,
          "specialid": 3102174,
          "global_collection_id": "collection_3_1085322503_53_0",
          "global_special_id": "collection_3_1085322503_53_0",
          "suid": 1085322503
        },
        "sub_title": "",
        "image_shape": 1,
        "position": 998,
        "subscript": 0,
        "type": 1
      },
      {
        "sizable_image_url": "http://c1.kgimg.com/custom/480/20260904/20260904132830732827.jpg_{size}x{size}.jpg",
        "id": 473761,
        "image_url": "http://c1.kgimg.com/custom/480/20260904/20260904132830732827.jpg",
        "title": "纪念何勇｜魔岩时代的麒麟少年",
        "extra": {
          "play_count": 3390,
          "specialid": 9808877,
          "global_collection_id": "collection_3_1002717860_553_0",
          "global_special_id": "collection_3_1002717860_553_0",
          "suid": 1002717860
        },
        "sub_title": "",
        "image_shape": 1,
        "position": 2,
        "subscript": 0,
        "type": 1
      },
      {
        "sizable_image_url": "http://mdpfilebssdlbig.kugou.com/6f79e157313dca0357799a29c5cffd3a.jpg_{size}x{size}.jpg",
        "id": 473363,
        "image_url": "http://mdpfilebssdlbig.kugou.com/6f79e157313d
  ...(已截断，完整数据见 responses/)
```

### /top/playlist

歌单榜

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| withtag | number | 否 | 1 | 是否带标签 |
| withsong | number | 否 | 1 | 是否带歌曲 |
| sort | number | 否 | 1 | 排序方式 |
| category_id | number | 否 | 0 | 分类ID |
| module_id | number | 否 | 1 | 模块ID |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ has_next: number
  └ bi_biz: string
  └ session: string
  └ alg_id: number
  └ special_list: array
  └ OlexpIds: string
  └ show_time: number
  └ all_client_playlist_flag: number
  └ refresh_time: number
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "data": {
    "has_next": 1,
    "bi_biz": "rcmd_spr",
    "session": "1788516883_2",
    "alg_id": 2,
    "special_list": [
      {
        "sync": 0,
        "specialid": 4670878,
        "flexible_cover": "http://c1.kgimg.com/custom/{size}/20220114/20220114224708558695.jpg",
        "list_info_trans_param": {
          "special_tag": 0,
          "iden": 8,
          "trans_flag": 1
        },
        "bz_status": 0,
        "singername": "群星",
        "percount": 0,
        "alg_path": "recall:hot",
        "from": -1,
        "tags": [
          {
            "tag_name": "流行",
            "tag_id": 9
          },
          {
            "tag_name": "英语",
            "tag_id": 20
          },
          {
            "tag_name": "游戏",
            "tag_id": 74
          }
        ],
        "ugc_talent_review": 1,
        "slid": 23,
        "type": 3,
        "nickname": "㞲",
        "show": "热门精选",
        "collectType": 0,
        "collectcount": 50368,
        "trans_param": {
          "special_tag": 0
        },
        "report_info": "t=1788516883,a=recall*hot",
        "specialname": "高燃』以凡人之躯，比肩神明。",
        "imgurl": "http://c1.kgimg.com/custom/{size}/20220114/20220114224708558695.jpg",
        "play_count": 27438418,
        "pic": "http://imge.kugou.com/kugouicon/165/20251005/20251005120227645846.jpg",
        "from_hash": "",
        "from_tag": -1,
        "publishtime": "2022-01-14 00:00:00",
        "global_collection_id": "collection_3_767478822_23_0",
        "intro": "",
        "suid": 767478822
      },
      {
        "sync": 0,
        "speciali
  ...(已截断，完整数据见 responses/)
```

### /top/song

新歌榜/顶部分类歌曲

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| type | number | 否 | 21608 | 榜单类型(21608新歌速递) |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
total: number
error_code: number
data: array  // array<object>，元素字段见下
  └ [].authors: array
  └ [].old_hide_flac: number
  └ [].status: number
  └ [].status_128: number
  └ [].filesize_320: number
  └ [].rp_id: number
  └ [].extern: string
  └ [].old_cpy_high: number
  └ [].cd_url: string
  └ [].fail_process: number
  └ [].pay_type: number
  └ [].rp_type: string
  └ [].topic_url_flac: string
  └ [].privilege_high: number
  └ [].parent_id: string
  └ [].old_cpy_320: number
  └ [].pkg_price_320: number
  └ [].topic_remark: string
  └ [].pay_type_high: number
  └ [].privilege_128: number
  └ [].price_320: number
  └ [].fail_process_128: number
  └ [].fail_process_320: number
  └ [].rank_id: string
  └ [].zone: string
  └ [].cid: number
  └ [].video_id: number
  └ [].buy_count: string
  └ [].hash: string
  └ [].rp_type_320: string
  └ [].price_128: number
  └ [].rp_type_flac: string
  └ [].timelength_high: number
  └ [].album_name: string
  └ [].topic_url_128: string
  └ [].issue: string
  └ [].publish_date: string
  └ [].ad_id: number
  └ [].timelength_super: number
  └ [].old_hide_super: number
  └ [].status_super: number
  └ [].old_hide_128: number
  └ [].pkg_price_super: number
  └ [].price_high: number
  └ [].timelength_128: number
  └ [].price_super: number
  └ [].max_sort: number
  └ [].exclusive: number
  └ [].album_audio_remark: string
  └ [].hash_super: string
  └ [].timelength: number
  └ [].privilege_super: number
  └ [].last_sort: number
  └ [].rp_type_128: string
  └ [].filesize_flac: number
  └ [].rank_count: number
  └ [].songname: string
  └ [].old_hide: number
  └ [].pay_type_128: number
  └ [].show_author_name: number
  └ [].video_hash: string
  └ [].recommend_reason: string
  └ [].rank_cid: number
  └ [].video_filesize: number
  └ [].status_320: number
  └ [].pay_type_super: number
  └ [].old_hide_320: number
  └ [].offset: number
  └ [].filename: string
  └ [].__status: number
  └ [].rp_type_high: string
  └ [].timelength_flac: number
  └ [].topic_url: string
  └ [].rp_publish: number
  └ [].old_cpy_super: number
  └ [].audio_id: number
  └ [].remark: string
  └ [].musical: object
  └ [].pkg_price_high: number
  └ [].filesize_128: number
  └ [].album_id: number
  └ [].hash_320: string
  └ [].pkg_price_flac: number
  └ [].has_obbligato: number
  └ [].topic_url_320: string
  └ [].fail_process_high: number
  └ [].filesize_high: number
  └ [].filesize: number
  └ [].extname_super: string
  └ [].status_high: number
  └ [].video_track: number
  └ [].bitrate_high: number
  └ [].bitrate_super: number
  └ [].hash_high: string
  └ [].price_flac: number
  └ [].author_name: string
  └ [].sort: number
  └ [].timelength_320: number
  └ [].album_sizable_cover: string
  └ [].extname: string
  └ [].topic_url_high: string
  └ [].status_flac: number
  └ [].privilege_flac: number
  └ [].old_cpy: number
  └ [].remarks: array
  └ [].old_cpy_128: number
  └ [].topic_url_super: string
  └ [].video_timelength: number
  └ [].filesize_super: number
  └ [].level: number
  └ [].privilege_320: number
  └ [].pkg_price_128: number
  └ [].price: number
  └ [].pay_type_flac: number
  └ [].rp_type_super: string
  └ [].pay_type_320: number
  └ [].bitrate: number
  └ [].trans_param: object
  └ [].hash_flac: string
  └ [].hash_128: string
  └ [].fail_process_super: number
  └ [].fail_process_flac: number
  └ [].old_cpy_flac: number
  └ [].old_hide_high: number
  └ [].album_audio_id: number
  └ [].privilege: number
  └ [].pkg_price: number
  └ [].bitrate_flac: number
  └ [].addtime: string
extra: object
status: number
errmsg: string
```

**返回示例**（截断）

```json
{
  "total": 100,
  "error_code": 0,
  "data": [
    {
      "authors": [
        {
          "sizable_avatar": "http://singerimg.kugou.com/uploadpic/softhead/{size}/20230531/20230531155603291987.jpg",
          "is_publish": 1,
          "author_id": 169967,
          "author_name": "周深"
        }
      ],
      "old_hide_flac": 1,
      "status": 0,
      "status_128": 0,
      "filesize_320": 9671771,
      "rp_id": 1108193620,
      "extern": "",
      "old_cpy_high": 0,
      "cd_url": "",
      "fail_process": 4,
      "pay_type": 3,
      "rp_type": "audio",
      "topic_url_flac": "",
      "privilege_high": 10,
      "parent_id": "1",
      "old_cpy_320": 0,
      "pkg_price_320": 1,
      "topic_remark": "",
      "pay_type_high": 3,
      "privilege_128": 8,
      "price_320": 200,
      "fail_process_128": 4,
      "fail_process_320": 4,
      "rank_id": "21608",
      "zone": "",
      "cid": 658997051,
      "video_id": 17553234,
      "buy_count": "0",
      "hash": "14ECD0A2A68CD70111C773430BEFBEF7",
      "rp_type_320": "audio",
      "price_128": 200,
      "rp_type_flac": "audio",
      "timelength_high": 241000,
      "album_name": "我们不散",
      "topic_url_128": "",
      "issue": "1",
      "publish_date": "2026-09-03",
      "ad_id": 470905,
      "timelength_super": 0,
      "old_hide_super": 0,
      "status_super": 1,
      "old_hide_128": 1,
      "pkg_price_super": 0,
      "price_high": 200,
      "timelength_128": 241000,
      "price_super": 0,
      "max_sort": 0,
      "exclusive": 0,
      "album_audio_remark": "《生逢其时》电视剧主题曲/片尾曲",
      "has
  ...(已截断，完整数据见 responses/)
```

## 评论

### /comment/album

专辑评论

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | string | 是 | - | 专辑/评论对象ID |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |
| show_classify | number | 否 | 1 | 显示分类 |
| show_hotword_list | number | 否 | 1 | 显示热词 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
status: number
err_code: number
msg: string
message: string
childrenid: string
count: number
combine_count: number
current_page: number
hasRcmd: boolean
song_score: object
  └ score_user_count: number
  └ score_list_url: string
  └ user_simple_song_scor: number
  └ is_show: number
  └ user_cmtid: string
  └ special_child_id: string
  └ special_child_name: string
  └ user_score_status: number
  └ song_score: number
  └ user_song_score: number
captcha: number
has_filter_risk_user: number
artCmtConfig: object
  └ tipOfNoUseArtCmt: string
  └ wayToEdit: number
  └ canUseArtCmt: number
  └ urlOfNoUseArtCmt: string
  └ h5EditorUrl: string
  └ canUploadImg: number
cache_time: number
config: object
  └ canUploadImg: number
  └ closeCmt: number
  └ emptyTip: string
  └ input_hint: string
  └ numOpusUserMax: number
  └ numOpusUserMaxSVIP: number
  └ numOpusUserMaxYVIP: number
  └ tipOfNoUploadImg: string
  └ tip_noupload_video: string
  └ toastBgOpus: string
  └ urlOfNoUploadImg: string
  └ url_noupload_video: string
  └ canUploadVoice: number
song_like_num: number
event_info: object
  └ event_url: string
  └ user_vote: number
  └ options: null
```

**返回示例**（截断）

```json
{
  "status": 1,
  "err_code": 0,
  "msg": "success",
  "message": "success",
  "childrenid": "1289478",
  "count": 0,
  "combine_count": 0,
  "current_page": 1,
  "hasRcmd": false,
  "song_score": {
    "score_user_count": 0,
    "score_list_url": "",
    "user_simple_song_scor": 0,
    "is_show": 0,
    "user_cmtid": "",
    "special_child_id": "",
    "special_child_name": "",
    "user_score_status": 0,
    "song_score": 0,
    "user_song_score": 0
  },
  "captcha": 0,
  "has_filter_risk_user": 0,
  "artCmtConfig": {
    "tipOfNoUseArtCmt": "该功能仅对部分用户开放",
    "wayToEdit": 1,
    "canUseArtCmt": 0,
    "urlOfNoUseArtCmt": "",
    "h5EditorUrl": "",
    "canUploadImg": 0
  },
  "cache_time": 0,
  "config": {
    "canUploadImg": 0,
    "closeCmt": 0,
    "emptyTip": "还没有人评论，快来抢沙发吧！",
    "input_hint": "把所有心事都放进评论里~",
    "numOpusUserMax": 3,
    "numOpusUserMaxSVIP": 1,
    "numOpusUserMaxYVIP": 3,
    "tipOfNoUploadImg": "该功能仅对部分用户开放",
    "tip_noupload_video": "该功能仅对部分用户开放",
    "toastBgOpus": "http://imge.kugou.com/commendpic/20190403/20190403110105584375.png",
    "urlOfNoUploadImg": "https://h5.kugou.com/requirement/v-c1f295e9/index.html",
    "url_noupload_video": "http://www2.kugou.kugou.com/apps/commentBlackHouse/msg.html?tid=126797095",
    "canUploadVoice": 0
  },
  "song_like_num": 0,
  "event_info": {
    "event_url": "",
    "user_vote": 0,
    "options": null
  }
}
```

### /comment/count

评论数量

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| hash | string | 否 | - | 歌曲hash(与special_id二选一) |
| special_id | string | 否 | - | 歌单ID(与hash二选一) |

**返回数据**

> 实测返回为业务错误（业务码 `None`），以下为上游真实错误信封；成功结构待在有效参数/环境下补充：

**返回字段结构**（基于真实返回推导）

```
e291fc24c9dbe4ef3fdd9837ea3e1a9c: number
```

**返回示例**（截断）

```json
{
  "e291fc24c9dbe4ef3fdd9837ea3e1a9c": 0
}
```

### /comment/floor

评论楼层（回复楼中楼）

> `tid` 为评论 ID（可从 /comment/music 返回的 `list[].id` 获取）；实测传入的 tid 与 mixsongid 不匹配时上游返回「歌曲不存在」。

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| special_id | string | 否 | - | 资源ID |
| resource_type | string | 否 | song | 资源类型 song/playlist/album |
| code | string | 否 | - | 签名码 |
| tid | string | 否 | - | 楼层ID |
| mixsongid | string | 否 | - | 歌曲mixsongid |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |
| show_classify | number | 否 | 1 | 显示分类 |
| show_hotword_list | number | 否 | 1 | 显示热词 |

**返回数据**

> 实测返回为业务错误（业务码 `0`），以下为上游真实错误信封；成功结构待在有效参数/环境下补充：

**返回字段结构**（基于真实返回推导）

```
err_code: number
msg: string
status: number
message: string
```

**返回示例**（截断）

```json
{
  "err_code": 10002,
  "msg": "歌曲不存在",
  "status": 0,
  "message": "歌曲不存在"
}
```

### /comment/music

歌曲评论

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| mixsongid | string | 是 | - | 歌曲mixsongid |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |
| show_classify | number | 否 | 1 | 显示分类 |
| show_hotword_list | number | 否 | 1 | 显示热词 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
status: number
err_code: number
msg: string
message: string
childrenid: string
count: number
combine_count: number
list: array  // array<object>，元素字段见下
  └ [].id: number
  └ [].special_id: number
  └ [].special_child_name: string
  └ [].code: string
  └ [].special_child_id: string
  └ [].status: string
  └ [].content: string
  └ [].bi: string
  └ [].puser_id: number
  └ [].pcontent: string
  └ [].delPid: number
  └ [].addtime: string
  └ [].action: string
  └ [].extdata: string
  └ [].reply_num: number
  └ [].user_id: number
  └ [].user_name: string
  └ [].user_pic: string
  └ [].user_sex: number
  └ [].userlimit: boolean
  └ [].vip_type: number
  └ [].y_type: number
  └ [].m_type: number
  └ [].vip_user_type: number
  └ [].album_audio_id: number
  └ [].is_publish: number
  └ [].jump: number
  └ [].song_show_text: string
  └ [].show_follow_button: number
  └ [].vinfo9: object
  └ [].vipinfo: object
  └ [].udetail: object
  └ [].like: object
  └ [].images: array
  └ [].edit_status: number
  └ [].encyclopedic_flag: number
  └ [].is_show_encyclopedic: number
  └ [].has_encyclopedic: number
  └ [].location: string
  └ [].is_hidden_cmt: number
  └ [].is_reply: number
  └ [].comments_num: number
  └ [].udetails: array
  └ [].album_id: number
  └ [].author_name: string
  └ [].cover: string
  └ [].hash: string
  └ [].loadoffset: number
  └ [].risk: number
  └ [].score: number
  └ [].loadone: number
  └ [].play_ts: number
  └ [].cmt_source: number
weightListTopLabel: string
current_page: number
weightListFullApi: string
hasRcmd: boolean
song_score: object
  └ score_user_count: number
  └ score_list_url: string
  └ user_simple_song_scor: number
  └ is_show: number
  └ user_cmtid: string
  └ special_child_id: string
  └ special_child_name: string
  └ user_score_status: number
  └ song_score: number
  └ user_song_score: number
captcha: number
has_filter_risk_user: number
hot_word_list: array  // array<object>，元素字段见下
  └ [].content: string
  └ [].count: number
artCmtConfig: object
  └ tipOfNoUseArtCmt: string
  └ wayToEdit: number
  └ canUseArtCmt: number
  └ urlOfNoUseArtCmt: string
  └ h5EditorUrl: string
  └ canUploadImg: number
tag: array  // array<object>，元素字段见下
  └ [].url: string
  └ [].name: string
  └ [].type: string
cache_time: number
classify_list: array  // array<object>，元素字段见下
  └ [].id: number
  └ [].label: string
  └ [].icon: string
  └ [].cnt: number
config: object
  └ canUploadImg: number
  └ closeCmt: number
  └ emptyTip: string
  └ input_hint: string
  └ numOpusUserMax: number
  └ numOpusUserMaxSVIP: number
  └ numOpusUserMaxYVIP: number
  └ tipOfNoUploadImg: string
  └ tip_noupload_video: string
  └ toastBgOpus: string
  └ urlOfNoUploadImg: string
  └ url_noupload_video: string
  └ canUploadVoice: number
topad: array  // array<object>，元素字段见下
  └ [].title: string
  └ [].jump: object
  └ [].picUrl: string
maxPage: number
song_like_num: number
music_can_push: number
event_info: object
  └ event_url: string
  └ user_vote: number
  └ options: null
tail_icon: array  // array<object>，元素字段见下
  └ [].id: string
  └ [].icon: string
popular_rank: object
  └ title: string
  └ sub_title: string
  └ jump_url: string
```

**返回示例**（截断）

```json
{
  "status": 1,
  "err_code": 0,
  "msg": "success",
  "message": "success",
  "childrenid": "92351310",
  "count": 160144,
  "combine_count": 160144,
  "list": [
    {
      "id": 1042969923,
      "special_id": 2364,
      "special_child_name": "温奕心 - 一路生花",
      "code": "fc4be23b4e972707f36b8a828a93ba8a",
      "special_child_id": "92351310",
      "status": "1",
      "content": "距离25考研24天，希望能如愿成为家里第一个211硕士研究生",
      "bi": "|_:1|images:1",
      "puser_id": 0,
      "pcontent": "",
      "delPid": 0,
      "addtime": "2024-11-27 23:06:41",
      "action": "",
      "extdata": "{\"city\":\"\\u5e7f\\u5dde\\u5e02\",\"action\":1,\"updatetime\":\"2026-09-04 17:16:13\"}",
      "reply_num": 132,
      "user_id": 644947101,
      "user_name": "反转魅力☆",
      "user_pic": "http://imge.kugou.com/kugouicon/165/20210712/20210712214329605263.jpg",
      "user_sex": 0,
      "userlimit": false,
      "vip_type": 0,
      "y_type": 0,
      "m_type": 0,
      "vip_user_type": 0,
      "album_audio_id": 317568292,
      "is_publish": 1,
      "jump": 1,
      "song_show_text": "温奕心 - 一路生花",
      "show_follow_button": 0,
      "vinfo9": {
        "auth_info": "",
        "pic": "http://imge.kugou.com/commendpic/20180627/20180627153930257837.png",
        "star_v_status": 0,
        "tme_star_status": 0,
        "actor_status": 0,
        "biz_status": 0,
        "cmt_talent_status": 1,
        "user_iden": 0,
        "student_status": 0,
        "user_label": "",
        "justiceLeague": 0,
        "singer_status": 0
      },
      "vipinfo": {
        "vip_type": 0,
        "y_type"
  ...(已截断，完整数据见 responses/)
```

### /comment/music/classify

歌曲评论分类（热评类型）

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| mixsongid | string | 是 | - | 歌曲mixsongid |
| type_id | string | 否 | - | 分类ID |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |
| sort | number | 否 | 1 | 排序方式 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
status: number
err_code: number
msg: string
message: string
childrenid: string
count: number
combine_count: number
current_page: number
list: null
config: object
  └ canUploadImg: number
  └ closeCmt: number
  └ emptyTip: string
  └ input_hint: string
  └ numOpusUserMax: number
  └ numOpusUserMaxSVIP: number
  └ numOpusUserMaxYVIP: number
  └ tipOfNoUploadImg: string
  └ tip_noupload_video: string
  └ toastBgOpus: string
  └ urlOfNoUploadImg: string
  └ url_noupload_video: string
  └ canUploadVoice: number
photo_list_style: number
```

**返回示例**（截断）

```json
{
  "status": 1,
  "err_code": 0,
  "msg": "success",
  "message": "success",
  "childrenid": "92351310",
  "count": 0,
  "combine_count": 0,
  "current_page": 1,
  "list": null,
  "config": {
    "canUploadImg": 0,
    "closeCmt": 0,
    "emptyTip": "还没有人评论，快来抢沙发吧！",
    "input_hint": "说点什么吧？",
    "numOpusUserMax": 3,
    "numOpusUserMaxSVIP": 1,
    "numOpusUserMaxYVIP": 3,
    "tipOfNoUploadImg": "该功能仅对部分用户开放",
    "tip_noupload_video": "该功能仅对部分用户开放",
    "toastBgOpus": "http://imge.kugou.com/commendpic/20190403/20190403110105584375.png",
    "urlOfNoUploadImg": "https://h5.kugou.com/requirement/v-c1f295e9/index.html",
    "url_noupload_video": "http://www2.kugou.kugou.com/apps/commentBlackHouse/msg.html?tid=126797095",
    "canUploadVoice": 0
  },
  "photo_list_style": 1
}
```

### /comment/music/hotword

评论热词

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| mixsongid | string | 是 | - | 歌曲mixsongid |
| hot_word | string | 否 | - | 热词 |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
current_hot_word: string
count: number
tag: array  // array<object>，元素字段见下
  └ [].url: string
  └ [].name: string
  └ [].type: string
childrenid: string
artCmtConfig: object
  └ tipOfNoUseArtCmt: string
  └ wayToEdit: number
  └ canUseArtCmt: number
  └ urlOfNoUseArtCmt: string
  └ h5EditorUrl: string
  └ canUploadImg: number
err_code: number
hot_word_list: array
msg: string
config: object
  └ tipOfNoUploadImg: string
  └ emptyTip: string
  └ urlOfNoUploadImg: string
  └ oppose_types: array
  └ input_hint: string
  └ opposebutton: number
  └ canUploadImg: number
  └ toastBgOpus: string
  └ closeCmt: number
  └ numOpusUserMaxSVIP: number
  └ can_upload_video: number
  └ tip_noupload_video: string
  └ numOpusUserMaxYVIP: number
  └ url_noupload_video: string
  └ numOpusUserMax: number
current_page: number
status: number
message: string
```

**返回示例**（截断）

```json
{
  "current_hot_word": "",
  "count": 0,
  "tag": [
    {
      "url": "http://m.comment.service.kugou.com/v1/get_hot_word?",
      "name": "热词",
      "type": "hotword"
    },
    {
      "url": "http://m.comment.service.kugou.com/v1/cmtlist?",
      "name": "推荐",
      "type": "default"
    },
    {
      "url": "http://m.comment.service.kugou.com/r/v1/rank/topliked?",
      "name": "最热",
      "type": "hot"
    },
    {
      "url": "http://m.comment.service.kugou.com/r/v1/rank/newest?",
      "name": "最新",
      "type": "newest"
    }
  ],
  "childrenid": "",
  "artCmtConfig": {
    "tipOfNoUseArtCmt": "该功能仅对部分用户开放喔",
    "wayToEdit": 1,
    "canUseArtCmt": 0,
    "urlOfNoUseArtCmt": "",
    "h5EditorUrl": "",
    "canUploadImg": 0
  },
  "err_code": 0,
  "hot_word_list": [],
  "msg": "success",
  "config": {
    "tipOfNoUploadImg": "该功能仅对部分用户开放",
    "emptyTip": "还没有人评论，快来抢沙发吧！",
    "urlOfNoUploadImg": "https://h5.kugou.com/requirement/v-c1f295e9/index.html",
    "oppose_types": [],
    "input_hint": "天青色等烟雨，而我在等你的评论",
    "opposebutton": 1,
    "canUploadImg": 0,
    "toastBgOpus": "http://imge.kugou.com/commendpic/20190403/20190403110105584375.png",
    "closeCmt": 0,
    "numOpusUserMaxSVIP": 1,
    "can_upload_video": 0,
    "tip_noupload_video": "该功能仅对部分用户开放",
    "numOpusUserMaxYVIP": 3,
    "url_noupload_video": "http://www2.kugou.kugou.com/apps/commentBlackHouse/msg.html?tid=126797095",
    "numOpusUserMax": 3
  },
  "current_page": 1,
  "status": 1,
  "message": "success"
}
```

### /comment/playlist

歌单评论

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | string | 是 | - | 歌单/歌曲ID |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |
| show_classify | number | 否 | 1 | 显示分类 |
| show_hotword_list | number | 否 | 1 | 显示热词 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
status: number
err_code: number
msg: string
message: string
childrenid: string
count: number
combine_count: number
current_page: number
hasRcmd: boolean
song_score: object
  └ score_user_count: number
  └ score_list_url: string
  └ user_simple_song_scor: number
  └ is_show: number
  └ user_cmtid: string
  └ special_child_id: string
  └ special_child_name: string
  └ user_score_status: number
  └ song_score: number
  └ user_song_score: number
captcha: number
has_filter_risk_user: number
artCmtConfig: object
  └ tipOfNoUseArtCmt: string
  └ wayToEdit: number
  └ canUseArtCmt: number
  └ urlOfNoUseArtCmt: string
  └ h5EditorUrl: string
  └ canUploadImg: number
cache_time: number
config: object
  └ canUploadImg: number
  └ closeCmt: number
  └ emptyTip: string
  └ input_hint: string
  └ numOpusUserMax: number
  └ numOpusUserMaxSVIP: number
  └ numOpusUserMaxYVIP: number
  └ tipOfNoUploadImg: string
  └ tip_noupload_video: string
  └ toastBgOpus: string
  └ urlOfNoUploadImg: string
  └ url_noupload_video: string
  └ canUploadVoice: number
song_like_num: number
event_info: object
  └ event_url: string
  └ user_vote: number
  └ options: null
```

**返回示例**（截断）

```json
{
  "status": 1,
  "err_code": 0,
  "msg": "success",
  "message": "success",
  "childrenid": "2550343002",
  "count": 0,
  "combine_count": 0,
  "current_page": 1,
  "hasRcmd": false,
  "song_score": {
    "score_user_count": 0,
    "score_list_url": "",
    "user_simple_song_scor": 0,
    "is_show": 0,
    "user_cmtid": "",
    "special_child_id": "",
    "special_child_name": "",
    "user_score_status": 0,
    "song_score": 0,
    "user_song_score": 0
  },
  "captcha": 0,
  "has_filter_risk_user": 0,
  "artCmtConfig": {
    "tipOfNoUseArtCmt": "该功能仅对部分用户开放",
    "wayToEdit": 1,
    "canUseArtCmt": 0,
    "urlOfNoUseArtCmt": "",
    "h5EditorUrl": "",
    "canUploadImg": 0
  },
  "cache_time": 0,
  "config": {
    "canUploadImg": 0,
    "closeCmt": 0,
    "emptyTip": "还没有人评论，快来抢沙发吧！",
    "input_hint": "这首歌让你想到什么？",
    "numOpusUserMax": 3,
    "numOpusUserMaxSVIP": 1,
    "numOpusUserMaxYVIP": 3,
    "tipOfNoUploadImg": "该功能仅对部分用户开放",
    "tip_noupload_video": "该功能仅对部分用户开放",
    "toastBgOpus": "http://imge.kugou.com/commendpic/20190403/20190403110105584375.png",
    "urlOfNoUploadImg": "https://h5.kugou.com/requirement/v-c1f295e9/index.html",
    "url_noupload_video": "http://www2.kugou.kugou.com/apps/commentBlackHouse/msg.html?tid=126797095",
    "canUploadVoice": 0
  },
  "song_like_num": 0,
  "event_info": {
    "event_url": "",
    "user_vote": 0,
    "options": null
  }
}
```

## 电台与 FM

### /fm/class

电台分类

**请求参数**：无

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
error_code: number
status: number
data: object
  └ update_time: number
  └ class_list: array
  └ class_total: number
```

**返回示例**（截断）

```json
{
  "error_code": 0,
  "status": 1,
  "data": {
    "update_time": 1788516600,
    "class_list": [
      {
        "classid": 10028,
        "fmlist": [
          {
            "imgurl_size": "480,400,240,150",
            "fmtype": 2,
            "banner_size": "480,400,240,150",
            "fmname": "散步",
            "broadcast_type": "1",
            "parentId": "0",
            "songlist": [
              {
                "hash": "0A31D13B63C14C220CA8863917077D9C",
                "tracker_info": {},
                "audio_id": 501404822,
                "m4asize": 0,
                "price_128": 0,
                "320size": 7669894,
                "hash_flac": "59E39314CC057525529AD0115A2D17C5",
                "type_320": "",
                "bitrate_flac": 925,
                "pay_type_320": 0,
                "size": 3067849,
                "pay_type_128": 0,
                "name": "夏楚楚 - 人生是场必修课",
                "fail_process": 4,
                "pay_type_flac": 0,
                "album_id": "",
                "ext": "mp3",
                "mvhash": "",
                "price_flac": 0,
                "type": "audio",
                "publish": 1,
                "320hash": "EA7281F1F2A09FC0C1071800E1D73437",
                "m4ahash": "",
                "pkg_price_flac": 0,
                "hash_ape": "",
                "privilege_flac": 0,
                "old_cpy": 0,
                "320time": 191000,
                "time": 191000,
                "filesize_flac": 22189570,
                "vip": 0,
                "type_128": "",
                
  ...(已截断，完整数据见 responses/)
```

### /fm/image

电台图片

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| fmid | string | 是 | - | FM ID，逗号分隔 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: array  // array<object>，元素字段见下
  └ [].imgUrl50: string
  └ [].fmid: number
  └ [].fmId: string
  └ [].imgUrl100: string
  └ [].fmtype: number
  └ [].imgUrl100_size: string
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "data": [
    {
      "imgUrl50": "20220128160910698183.jpg",
      "fmid": 1,
      "fmId": "1",
      "imgUrl100": "http://imge.kugou.com/fmlogo/{size}/20220128160903915157.jpg",
      "fmtype": 2,
      "imgUrl100_size": "145,64,60"
    }
  ],
  "status": 1,
  "error_code": 0
}
```

### /fm/recommend

电台推荐

**请求参数**：无

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: array  // array<object>，元素字段见下
  └ [].classid: string
  └ [].isnew: string
  └ [].rcmdlist: array
  └ [].classname: string
  └ [].broadcast_type: string
  └ [].description: string
  └ [].position: string
  └ [].offset: number
  └ [].fmtype: number
  └ [].banner_size: string
  └ [].fmid: string
  └ [].addtime: string
  └ [].imgurl_size: string
  └ [].parentId: string
  └ [].fmname: string
  └ [].rcmdsongsize: number
  └ [].banner: string
  └ [].heat: string
  └ [].imgurl: string
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "data": [
    {
      "classid": "7",
      "isnew": "0",
      "rcmdlist": [
        {
          "hash": "A45801CDCA935CF9B2EC25FBB8682357",
          "bitrate": 128,
          "m4asize": 0,
          "price_128": 0,
          "320size": 8658285,
          "hash_flac": "B0C09D33F27E5B1CE9706D6C6EE91253",
          "type_320": "",
          "bitrate_flac": 0,
          "pay_type_320": 0,
          "size": 3463451,
          "pay_type_128": 0,
          "name": "Unlike Pluto、Why Mona、Joanna Jones - Wannabe",
          "fail_process": 0,
          "pay_type_flac": 0,
          "album_id": "",
          "ext": "mp3",
          "mvhash": "",
          "price_flac": 0,
          "sid": 2520763,
          "type": "audio",
          "publish": 1,
          "320hash": "20C4004F448945B190815FE33C09127E",
          "bitrate_ape": 0,
          "album_audio_id": 362323923,
          "hash_ape": "",
          "privilege_flac": 0,
          "old_cpy": 1,
          "320time": 216432,
          "time": 216424,
          "audio_id": 27990163,
          "pay_type": 0,
          "type_128": "",
          "pkg_price_320": 0,
          "privilege_320": 0,
          "pkg_price_128": 0,
          "privilege_128": 0,
          "price_320": 0,
          "privilege": 0,
          "fail_process_128": 0,
          "fail_process_320": 0,
          "trans_param": {
            "pay_block_tpl": 1,
            "classmap": {
              "attr0": 234881032
            },
            "language": "英语",
            "cpy_attr0": 50339840,
            "musicpack_advance": 0,
            "ogg_128_filesize":
  ...(已截断，完整数据见 responses/)
```

### /fm/songs

电台歌曲列表

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| fmid | string | 是 | - | FM ID，逗号分隔 |
| type | number | 否 | 2 | fm类型 |
| offset | number | 否 | -1 | 偏移 |
| size | number | 否 | 20 | 数量 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: array  // array<object>，元素字段见下
  └ [].size: number
  └ [].fmid: number
  └ [].songs: array
  └ [].offset: number
  └ [].fmtype: number
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "data": [
    {
      "size": 20,
      "fmid": 1,
      "songs": [
        {
          "type_flac": "",
          "filesize_ape": 0,
          "filesize_flac": 25663675,
          "privilege_320": 10,
          "hmvhash": "",
          "pkg_price_flac": 1,
          "hash_super": "",
          "hashflac": "187938268128467B6F4394A4DE8C33EB",
          "sid": 317847687,
          "price_128": 200,
          "m4ahash": "",
          "_is_recommend": 0,
          "filesize_super": 0,
          "type_320": "",
          "trac": 0,
          "album_audio_id": 514945580,
          "pay_type_320": 3,
          "size": 3793888,
          "pay_type_128": 3,
          "mvtype": 0,
          "m4asize": 0,
          "name": "王超然 - 来人间走个过场",
          "fail_process": 4,
          "pay_type": 3,
          "pay_type_flac": 3,
          "vip": 0,
          "album_id": 73308440,
          "ext": "mp3",
          "mvhash": "",
          "price_flac": 200,
          "320time": 237087,
          "type": "audio",
          "musictrac": 0,
          "320hash": "C24F561BDB09391DFA138689EC815F0F",
          "isfilehead": 100,
          "hash_high": "",
          "hash_ape": "",
          "privilege_flac": 10,
          "old_cpy": 0,
          "publish": 1,
          "time": 237087,
          "ismvfilehead": 1,
          "mv": "",
          "type_128": "",
          "pkg_price_320": 1,
          "topic_remark": "",
          "pkg_price_128": 1,
          "privilege_128": 10,
          "price_320": 200,
          "bitrate": 128,
          "fail_process_128": 4,
          "fail_process_320": 4,
 
  ...(已截断，完整数据见 responses/)
```

### /pc/diantai

PC 电台

**请求参数**：无

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
status: number
error_code: number
data: object
  └ timestamp: number
  └ config: object
  └ data: array
  └ timing_update_interval: number
```

**返回示例**（截断）

```json
{
  "status": 1,
  "error_code": 0,
  "data": {
    "timestamp": 1788516909,
    "config": {
      "close_hotzone": 1
    },
    "data": [
      {
        "sort": 14,
        "code": "http://adsfile.bssdlbig.kugou.com/d06be8970e9f8ad799abeb9ae39f0d67.jpg",
        "id": 248396,
        "isAd": true,
        "jump_type": 6,
        "title": "虚拟直播",
        "type": 1,
        "url": "https://fanxing.kugou.com/ether/transitPage.html?cid=30&randomNum=5",
        "tongji": 0
      },
      {
        "sort": 12,
        "code": "http://adsfile.bssdlbig.kugou.com/eb559a3a31df15a6444be98a7c22da2d.jpg",
        "id": 248395,
        "isAd": true,
        "jump_type": 6,
        "title": "舞蹈直播",
        "type": 1,
        "url": "https://fanxing.kugou.com/ether/transitPage.html?cid=24&randomNum=5",
        "tongji": 0
      },
      {
        "sort": 11,
        "code": "http://adsfile.bssdlbig.kugou.com/f3c12ebba14cf5d4842fe9f8ca4ca3a3.jpg",
        "id": 298223,
        "isAd": true,
        "jump_type": 0,
        "title": "HiFi",
        "type": 1,
        "url": "https://www.kugou.com/hifi/",
        "tongji": 0
      },
      {
        "sort": 10,
        "code": "http://adsfile.bssdlbig.kugou.com/44802dda96e0c152f0badf570e911b7b.jpg",
        "id": 248392,
        "isAd": true,
        "jump_type": 6,
        "title": "游戏直播",
        "type": 1,
        "url": "https://fanxing.kugou.com/ether/transitPage.html?cid=6201&randomNum=15",
        "tongji": 0
      },
      {
        "sort": 10,
        "code": "http://mdpfilebssdlbig.kugou.com/1162e87e6fb040ab2468aaa4338b1d7f.jpg",
 
  ...(已截断，完整数据见 responses/)
```

### /personal/fm

私人 FM

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| action | string | 否 | play | 动作 play/skip |
| song_pool_id | string | 否 | 0 | 歌曲池ID |
| platform | string | 否 | ios | 平台 |
| remain_songcnt | string | 否 | 0 | 剩余歌曲数 |
| mode | string | 否 | normal | 模式 |
| is_overplay | boolean | 否 | false | 是否超播 |
| hash | string | 否 | - | 上一首hash |
| songid | string | 否 | - | 上一首songid |
| playtime | string | 否 | - | 上一首播放时长 |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

## 推荐与首页

### /ai/recommend

AI 推荐

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| album_audio_id | string | 否 | - | 推荐种子歌曲ID，逗号分隔 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ is_clean: number
  └ song_list: array
  └ sync_point: number
  └ clientver: string
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "data": {
    "is_clean": 0,
    "song_list": [
      {
        "mv_hash": "",
        "hash": "8E411151F802A075AC698E60450C3E6E",
        "singerinfo": [
          {
            "name": "王宇良",
            "is_publish": "1",
            "id": "195812"
          }
        ],
        "official_songname": "",
        "bitrate": 128,
        "ori_audio_name": "世间",
        "hash_192": "",
        "filesize_320": 7554380,
        "filesize_128": 3022034,
        "songname": "世间",
        "hash_other": "",
        "rec_song_info": {
          "similar_desc": "低",
          "alg_path": "hotsong,g=2",
          "songid": "0",
          "rec_desc2": "相似度：低",
          "rec_desc": "正在随机推荐歌曲"
        },
        "songid": 9396446,
        "quality_level": 3,
        "fail_process": 0,
        "filesize_192": 0,
        "song_type": "1",
        "album_id": "1992705",
        "has_album": 1,
        "extname": "mp3",
        "type": "audio",
        "remark": "",
        "hash_320": "3C8DE51F915D9865F7F80FCCC1259B93",
        "level": 2,
        "time_length": 188,
        "music_trac": 0,
        "old_cpy": 1,
        "pay_type": 0,
        "language": "国语",
        "is_file_head_320": 100,
        "relate_goods": [
          {
            "quality": "128",
            "hash": "8E411151F802A075AC698E60450C3E6E",
            "discount": {},
            "rebuy_pay_type": 0,
            "old_hide": 0,
            "is_publish": 1,
            "is_search_top": 0,
            "name": "王宇良 - 世间",
            "fail_process": 0,
            "_errno": 0,
            "album_id": "1992705",
  
  ...(已截断，完整数据见 responses/)
```

### /brush

刷一刷（随机推荐）

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| mode | string | 否 | normal | 模式 normal |
| song_pool_id | number | 否 | 0 | 歌曲池ID |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
timestamp: number
status: number
errcode: number
data: object
  └ list: array
  └ total: number
  └ pool_type: number
  └ user_tags: array
  └ has_next: number
  └ paging: object
  └ next_page: number
error: string
```

**返回示例**（截断）

```json
{
  "timestamp": 1788516840,
  "status": 1,
  "errcode": 0,
  "data": {
    "list": [
      {
        "song": {
          "wide_audio_id": 60601056,
          "is_publish": 1,
          "author_name": "Bvrnout、AXYL",
          "audio_id": 60601056,
          "author_cover": "http://singerimg.kugou.com/uploadpic/softhead/{size}/20250319/20250319072337487676.jpg",
          "album_audio_id": 220716723,
          "album_id": 30210059,
          "album_cover": "http://imge.kugou.com/stdmusic/{size}/20230115/20230115083002412303.jpg",
          "songname": "Silhouette"
        },
        "hp_mv_info": {
          "play_info": {
            "expire_time": 1788603240,
            "src_file": {
              "h265": {
                "backup_url": [],
                "play_url": "http://explorebssdlbig.kugou.com/202609041814/29c43ee2703e8ceab011381b8386544e/f0370cb81a6dc2e936355a28b571b06e.mp4"
              },
              "h264": {
                "backup_url": [],
                "play_url": "http://explorebssdlbig.kugou.com/202609041814/c8e9fea637e4e480b06ae999e9dcc4f6/1c1c33cbdbde7f1659344adbd332a09d.mp4"
              }
            }
          },
          "mv_info": {
            "is_show": 1,
            "theme_id": 62,
            "video_list": [
              {
                "video_id": "55c7eb809d81ada37955dd541fe2968d",
                "e_time": 209,
                "likes": 0,
                "video_bss_second_img": "",
                "s_time": 0,
                "views": 0,
                "video_bss_img": "https://coolimgbssdl.kugou.com/f06fb39a9956796e04ce636bea
  ...(已截断，完整数据见 responses/)
```

### /everyday/friend

每日好友（好友在听）

**请求参数**：无

**返回数据**

> 实测返回为业务错误（业务码 `None`），以下为上游真实错误信封；成功结构待在有效参数/环境下补充：

**返回字段结构**（基于真实返回推导）

```
data: string
```

**返回示例**（截断）

```json
{
  "data": ""
}
```

### /everyday/history

每日推荐历史

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| mode | string | 否 | list | 模式 list |
| platform | string | 否 | ios | 平台 |
| history_name | string | 否 | - | 历史名称 |
| date | string | 否 | - | 日期 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ OlexpIds: string
  └ bi_biz: string
  └ mode: string
  └ lists: array
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "data": {
    "OlexpIds": "30,2218,2264,2270,2304,2316,2358,2866,2908,2910,2952,2954,2966,2968,2992,2994,6917,6919,7693,7695,7697,7699,7789,7791,7813,7815,7849,7851,8129,8133,8159,8161,8181,8281,8283,8297,8299,8345,8347,8383,8385,8513,8525,8527,8599,8601,8625,8627,8791,8793,8855,8857,9191,9575,9577,10621,10623",
    "bi_biz": "rcmd_evd",
    "mode": "list",
    "lists": [
      {
        "song_list": [
          {
            "filesize_flac": 28079445,
            "official_songname": "",
            "ori_audio_name": "化风行万里",
            "hash_192": "",
            "hash_flac": "8C9B2CA43FE96517C29DDD1993BA91D7",
            "songname": "化风行万里",
            "music_trac": 1,
            "is_original": 2,
            "pay_type": 3,
            "song_type": "1",
            "album_id": "78165497",
            "remark": "",
            "language": "国语",
            "is_file_head_320": 100,
            "alg_path": "recall_v2_alg=a=0,rn=888_ydg&evd_ydg,crn=go_server;itid=cee8d7c5-25e1-133c-4a91-5c1ad86ab284",
            "is_file_head": 100,
            "filename": "洋澜一 - 化风行万里",
            "cid": 0,
            "scid": 278476213,
            "suffix_audio_name": "",
            "mv_hash": "9796AF374B8FAB01605EC00289C64B24",
            "hash": "5BF9192C2F0888E2A5B1EE6786E449C4",
            "author_name": "洋澜一",
            "tracker_info": {
              "auth": "48d58e7176104a7f7b8d1b99ea0bea8e",
              "module_id": 10,
              "open_time": "20260904"
            },
            "bitrate": 128,
            "is_mv_file_head": 1,
            "has_accompany": 0,
  ...(已截断，完整数据见 responses/)
```

### /everyday/recommend

每日歌曲推荐

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| platform | string | 否 | ios | 平台 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ creation_date: string
  └ mid: string
  └ bi_biz: string
  └ sign: string
  └ song_list_size: number
  └ OlexpIds: string
  └ client_playlist_flag: number
  └ is_guarantee_rec: number
  └ song_list: array
  └ sub_title: string
  └ cover_img_url: string
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "data": {
    "creation_date": "20260904",
    "mid": "66759400758721805038086495813949956634",
    "bi_biz": "rcmd_evd",
    "sign": "47db2fdb7e622836c0ade5d58f0a62d4",
    "song_list_size": 30,
    "OlexpIds": "30,2218,2264,2270,2304,2316,2358,2866,2908,2910,2952,2954,2966,2968,2992,2994,6917,6919,7693,7695,7697,7699,7789,7791,7813,7815,7849,7851,8129,8133,8159,8161,8181,8281,8283,8297,8299,8345,8347,8383,8385,8513,8525,8527,8599,8601,8625,8627,8791,8793,8855,8857,9191,9575,9577,10621,10623",
    "client_playlist_flag": 0,
    "is_guarantee_rec": 0,
    "song_list": [
      {
        "filesize_flac": 22796628,
        "official_songname": "",
        "ori_audio_name": "其实你不是没爱过",
        "hash_192": "",
        "hash_flac": "5804B30BFB8F2B22A7D1FFF20F84D0F9",
        "songname": "其实你不是没爱过",
        "music_trac": 1,
        "is_original": 0,
        "pay_type": 3,
        "song_type": "1",
        "album_id": "153443545",
        "remark": "",
        "language": "国语",
        "is_file_head_320": 100,
        "alg_path": "t=1788516846;recall_v2_alg=a=0,rn=99-97_ydg&evd_ydg,crn=go_server;itid=1cad593e-545c-e242-7a83-6f682edffe36",
        "is_file_head": 100,
        "filename": "阿珞 - 其实你不是没爱过",
        "cid": 0,
        "scid": 472503050,
        "suffix_audio_name": "",
        "rec_copy_write": "人气歌曲推荐",
        "mv_hash": "6994998DE33B04B8371B888113283B96",
        "hash": "9A14E5533601817E9D3CE438CCAF4B1D",
        "author_name": "阿珞",
        "tags": [
          {
            "tag_id": "84",
            "parent_id": "7",
            "tag_name": "国语"
          },
 
  ...(已截断，完整数据见 responses/)
```

### /everyday/style/recommend

每日风格推荐

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| tagids | string | 否 | - | 风格标签ID |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ mid: string
  └ bi_biz: string
  └ sign: string
  └ song_list_size: number
  └ title: string
  └ tagids: string
  └ client_playlist_flag: number
  └ song_list: array
  └ OlexpIds: string
  └ tag_info: array
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "data": {
    "mid": "66759400758721805038086495813949956634",
    "bi_biz": "rcmd_evd",
    "sign": "1a876991fc0569e5b37d909d52a0a835",
    "song_list_size": 30,
    "title": "华语流行",
    "tagids": "S14",
    "client_playlist_flag": 0,
    "song_list": [
      {
        "filesize_flac": 32170188,
        "official_songname": "",
        "ori_audio_name": "醉赤壁",
        "hash_192": "",
        "hash_flac": "E54FFFF754D234B7116F0531CB0C180F",
        "songname": "醉赤壁",
        "music_trac": 1,
        "fail_process": 4,
        "pay_type": 3,
        "filesize_other": 0,
        "album_id": "963179",
        "remark": "",
        "language": "国语",
        "is_file_head_320": 100,
        "alg_path": "t=1788516846;fr=1;itid=811eeb12-49ec-e65d-7e6e-f4812f9ac78b;citid=;a=4;b=7",
        "climax_timelength": 31600,
        "timelength_320": 281,
        "cid": 0,
        "scid": 322676,
        "suffix_audio_name": "",
        "rec_copy_write": "人气歌曲推荐",
        "mv_hash": "272A4874EBD4CB8DE5EDDC3BCF0EFC14",
        "hash": "51ABC8CF6D25ED868B989EEF7632F975",
        "singerinfo": [
          {
            "is_publish": "1",
            "name": "林俊杰",
            "id": "1574"
          }
        ],
        "sizable_cover": "http://imge.kugou.com/stdmusic/{size}/20241118/20241118160625225560.jpg",
        "rank_label": "“充满古典气息”",
        "bitrate": 128,
        "is_mv_file_head": 1,
        "has_accompany": 1,
        "filesize_128": 4505794,
        "hash_other": "",
        "is_publish": 1,
        "climax_end_time": 208600,
        "songid": 16580,
        "quality_level":
  ...(已截断，完整数据见 responses/)
```

### /images

图片素材

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| hash | string | 是 | - | 图片hash，逗号分隔 |
| album_id | string | 否 | 0 | 专辑ID，逗号分隔 |
| album_audio_id | string | 否 | 0 | 音频ID，逗号分隔 |
| count | number | 否 | 5 | 数量 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
status: number
error_code: number
errcode: number
errmsg: string
data: array  // array<object>，元素字段见下
  └ [].author: array
  └ [].album: array
```

**返回示例**（截断）

```json
{
  "status": 1,
  "error_code": 0,
  "errcode": 0,
  "errmsg": "",
  "data": [
    {
      "author": [],
      "album": []
    }
  ]
}
```

### /images/audio

音频图片（封面等）

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| hash | string | 是 | - | 音频hash，逗号分隔 |
| audio_id | string | 否 | 0 | 音频ID，逗号分隔 |
| album_audio_id | string | 否 | 0 | 音频ID，逗号分隔 |
| filename | string | 否 | - | 文件名 |
| count | number | 否 | 5 | 数量 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
status: number
error_code: number
errcode: number
errmsg: string
data: array
```

**返回示例**（截断）

```json
{
  "status": 1,
  "error_code": 0,
  "errcode": 0,
  "errmsg": "",
  "data": [
    []
  ]
}
```

### /recommend/songs

推荐歌曲

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| platform | string | 否 | android | 平台 |
| userid | string | 否 | 0 | 用户ID |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ creation_date: string
  └ mid: string
  └ bi_biz: string
  └ sign: string
  └ song_list_size: number
  └ OlexpIds: string
  └ client_playlist_flag: number
  └ is_guarantee_rec: number
  └ song_list: array
  └ sub_title: string
  └ cover_img_url: string
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "data": {
    "creation_date": "20260904",
    "mid": "",
    "bi_biz": "rcmd_evd",
    "sign": "4436e899dd0ee45ec3d7f987895eec43",
    "song_list_size": 29,
    "OlexpIds": "30,2218,2264,2270,2304,2316,2358,2866,2908,2910,2952,2954,2966,2968,2992,2994,6917,6919,7693,7695,7697,7699,7789,7791,7813,7815,7849,7851,8129,8133,8159,8161,8181,8281,8283,8297,8299,8345,8347,8383,8385,8513,8525,8527,8599,8601,8625,8627,8791,8793,8855,8857,9191,9575,9577,10621,10623",
    "client_playlist_flag": 0,
    "is_guarantee_rec": 0,
    "song_list": [
      {
        "filesize_flac": 24075342,
        "official_songname": "",
        "ori_audio_name": "黑桃A",
        "hash_192": "",
        "hash_flac": "64BB4E6998E2CE48F78E2B25D29B319A",
        "songname": "黑桃A",
        "music_trac": 1,
        "is_original": 1,
        "pay_type": 3,
        "song_type": "1",
        "album_id": "48003029",
        "remark": "",
        "language": "国语",
        "is_file_head_320": 100,
        "alg_path": "t=1788516863;b=7;recall_v2_alg=citid=18143eaa-ba72-aece-8c97-5445e53a3242,de14ee17-9fae-52f4-ce2b-3f67b33dd737,e3759e9b-f7ea-c3f2-da9e-4a4c349e613e,931aeb3a-6145-ee87-3dbc-eeac48d24332,472de6f3-3c5c-2ea1-3983-6aa1f6a2fa67,eeec232c-aced-a,crn=go_server;itid=ac3932d7-cdce-5cad-27aa-8327e574ec54",
        "is_file_head": 100,
        "filename": "DJ小鱼儿 - 黑桃A",
        "cid": 0,
        "scid": 116726216,
        "suffix_audio_name": "",
        "rec_copy_write": "人气歌曲推荐",
        "mv_hash": "319125ECD143520FB32829DD7FE19EFB",
        "hash": "095C509C195C92EEA0E52369A25EF91A",
        "author_name": "D
  ...(已截断，完整数据见 responses/)
```

### /scene/audio/list

场景下音频列表

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | string | 是 | - | 场景ID |
| module_id | string | 是 | - | 模块ID |
| tag | string | 是 | - | 标签 |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |

**返回数据**

> 实测返回为业务错误（业务码 `0`），以下为上游真实错误信封；成功结构待在有效参数/环境下补充：

**返回字段结构**（基于真实返回推导）

```
errcode: number
status: number
error: string
```

**返回示例**（截断）

```json
{
  "errcode": 40001,
  "status": 0,
  "error": "参数错误"
}
```

### /scene/collection/list

场景合集列表

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| tag_id | string | 是 | - | 标签ID |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ timestamp: number
  └ list: array
errcode: number
status: number
error: string
```

**返回示例**（截断）

```json
{
  "data": {
    "timestamp": 1788516885,
    "list": [
      {
        "is_publish": 1,
        "id": "collection_3_1789714925_7_0",
        "specialid": 6940596,
        "song_count": 31,
        "author_info": {
          "user_pic": "http://imge.kugou.com/kugouicon/165/20231026/20231026013005204545.jpg",
          "user_name": "katrina",
          "user_id": 1789714925
        },
        "content_id": "collection_3_1789714925_7_0",
        "play_count": 153928,
        "title": "上头减脂！一起动起来",
        "intro": "最适合健身房的歌单！大佬必备！",
        "cover": "http://imge.kugou.com/soft/collection/{size}/20230803/20230803164602753012.jpg",
        "slid": 7,
        "create_time": 1690959796,
        "content_type": 1,
        "song_userid": 1789714925
      },
      {
        "is_publish": 1,
        "id": "collection_3_1789714925_5_0",
        "specialid": 6940457,
        "song_count": 29,
        "author_info": {
          "user_pic": "http://imge.kugou.com/kugouicon/165/20231026/20231026013005204545.jpg",
          "user_name": "katrina",
          "user_id": 1789714925
        },
        "content_id": "collection_3_1789714925_5_0",
        "play_count": 224338,
        "title": "「节奏控」健身必备",
        "intro": "踏着轻捷的舞步，展现出无限活力；挥舞强健...",
        "cover": "http://imge.kugou.com/soft/collection/{size}/20230803/20230803181056718192.jpg",
        "slid": 5,
        "create_time": 1690957997,
        "content_type": 1,
        "song_userid": 1789714925
      },
      {
        "is_publish": 1,
        "id": "collection_3_1789714925_9_0",
        "specialid": 6940695,
        "song_count"
  ...(已截断，完整数据见 responses/)
```

### /scene/lists

场景列表

**请求参数**：无

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ timestamp: number
  └ is_scene_grey_user: boolean
  └ list: array
errcode: number
status: number
error: string
```

**返回示例**（截断）

```json
{
  "data": {
    "timestamp": 1788516885,
    "is_scene_grey_user": false,
    "list": [
      {
        "label": "催眠/纯音/雨声",
        "is_publish": 1,
        "music_tag": {
          "module_id": 595,
          "module_sort": 1,
          "module_extra": [
            {
              "quku_tag_id": 94301,
              "sort": 1,
              "tag_name": "快速入睡",
              "pic": "http://mobileservice.bssdl.kugou.com/014ccd5c100b9f5f5f052bd9bafa41d7.jpg",
              "play_order": 2,
              "abtest": "",
              "tag_id": 42907,
              "content_type": 6,
              "select_tag": 0
            },
            {
              "quku_tag_id": 94457,
              "sort": 2,
              "tag_name": "纯音乐",
              "pic": "http://mobileservice.bssdl.kugou.com/2e2e4e83c80b421c8b5d88469f9d6d9d.jpg",
              "play_order": 2,
              "abtest": "",
              "tag_id": 42923,
              "content_type": 6,
              "select_tag": 0
            },
            {
              "quku_tag_id": 94302,
              "sort": 3,
              "tag_name": "催眠脑波",
              "pic": "http://mobileservice.bssdl.kugou.com/4c714c375f6004d3a0d19f100b13a24b.jpg",
              "play_order": 2,
              "abtest": "",
              "tag_id": 42702,
              "content_type": 6,
              "select_tag": 0
            },
            {
              "quku_tag_id": 93476,
              "sort": 5,
              "tag_name": "雨声",
              "pic": "http://mobileservice.bssdl.kugou.com/2a2f3334edf2f030bb961b6b57e6a9c6.jpg",
            
  ...(已截断，完整数据见 responses/)
```

### /scene/lists/v2

场景列表（v2）

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | string | 是 | - | 场景ID |
| sort | string | 否 | rec | 排序 rec/hot/new |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
timestamp: number
status: number
errcode: number
data: object
  └ list: array
  └ total: number
  └ rec_list: array
  └ has_next: number
  └ paging: object
  └ next_page: number
error: string
```

**返回示例**（截断）

```json
{
  "timestamp": 1788516886,
  "status": 1,
  "errcode": 0,
  "data": {
    "list": [
      {
        "song": {
          "hash_320": "",
          "hash": "",
          "album_cover": "",
          "privilege_flac": 0,
          "audio_id": 0,
          "privilege_128": 0,
          "author_name": "",
          "hash_flac": "",
          "privilege_320": 0,
          "extname": "",
          "songname": ""
        },
        "topic": null,
        "comment_total": 0,
        "detail_url": "",
        "g_visible_type": 0,
        "g_created_location": "广东",
        "collection_type": "cloudlist_collection_info",
        "updated_at": "2026-03-24 10:59:01",
        "topic_type": "scene_music_topic_detail",
        "released_at": "2026-03-24 10:59:10",
        "g_location": "广东",
        "created_at": "2026-03-24 10:59:01",
        "userid": 2172418479,
        "content_ver": 1,
        "reviewed_at": "2026-03-24 10:59:10",
        "deleted_at": null,
        "is_anonymous": 0,
        "g_user": {
          "userid": 2172418479,
          "nickname": "2172418479",
          "avatar": "http://imge.kugou.com/kugouicon/165/20240329/20240329134549786535.jpg"
        },
        "haslike": 0,
        "song_type": "kmr_v1_audio",
        "content": "1269",
        "g_not_rec": 0,
        "collection": null,
        "id": "1489821569660647299",
        "like_total": 0,
        "scene_id": 1
      },
      {
        "song": {
          "filesize_flac": 26759696,
          "author_name": "郭一凡",
          "privilege_flac": 0,
          "audio_id": 62306801,
          "filesize_320": 109
  ...(已截断，完整数据见 responses/)
```

### /scene/module

场景模块

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | string | 是 | - | 场景ID |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ timestamp: number
  └ is_scene_grey_user: boolean
  └ content: object
errcode: number
status: number
error: string
```

**返回示例**（截断）

```json
{
  "data": {
    "timestamp": 1788516887,
    "is_scene_grey_user": false,
    "content": {
      "match_tag_info": null,
      "match_content_info": [],
      "match_type": 0,
      "select_tag": [],
      "music_station": {
        "ip_id": 91540,
        "userid": 1085324644
      },
      "info": {
        "play_tag_type": 6,
        "play_tag_id": 42691,
        "icon": "http://mobileservice.bssdl.kugou.com/c636a507bdff78e2431eed1fc041c642.png",
        "play_quku_tag_id": 0,
        "bg_pic": "http://mobileservice.bssdl.kugou.com/8d793a52771d257ab35543c85002f6b3.jpeg",
        "title_pic": "http://mobileservicebssdl.kugou.com/9ff5560c58d9516ae913f2a52ca30897.png",
        "label": "催眠/纯音/雨声",
        "is_publish": 1,
        "search_tag_id": 42921,
        "sort": 1,
        "popup": "助眠",
        "search_module_id": 0,
        "search_quku_tag_id": 0,
        "title": "助眠专区",
        "color": "#256688",
        "search_tag_type": 1,
        "pic": "http://mobileservice.bssdl.kugou.com/e229c20d906c5f8795659e57f826cfb7.jpg",
        "extra": {
          "view_num": 18266723,
          "song_count": 1134
        },
        "intro": "",
        "scene_id": 1
      },
      "discuss": [
        {
          "tab": [],
          "abtest": "",
          "mini_programs": [],
          "module_id": 53,
          "line": 2,
          "label": "",
          "module_type": 4,
          "sort": 999,
          "module_style": 1,
          "title": "大家在聊",
          "bottom_line": 1,
          "parent_module_id": 0,
          "need_login": 0,
          "personal_recommend": 0,
    
  ...(已截断，完整数据见 responses/)
```

### /scene/module/info

场景模块信息

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | string | 是 | - | 场景ID |
| module_id | string | 是 | - | 模块ID |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ timestamp: number
  └ info: array
errcode: number
status: number
error: string
```

**返回示例**（截断）

```json
{
  "data": {
    "timestamp": 1788516887,
    "info": []
  },
  "errcode": 0,
  "status": 1,
  "error": ""
}
```

### /scene/music

场景音乐

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | string | 是 | - | 场景ID |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
timestamp: number
status: number
errcode: number
data: object
  └ list: array
  └ total: number
  └ has_next: number
  └ next_page: number
error: string
```

**返回示例**（截断）

```json
{
  "timestamp": 1788516888,
  "status": 1,
  "errcode": 0,
  "data": {
    "list": [
      {
        "song": {
          "filesize_flac": 15139693,
          "author_name": "饭碗的彼岸",
          "privilege_flac": 10,
          "bitrate": 128,
          "hash_flac": "82805DA3EE29F9E101488D7B9BF172B8",
          "privilege_320": 10,
          "privilege_128": 8,
          "hash_320": "158E479CF57E19063C42229CE6A0538F",
          "is_publish": 1,
          "album_cover": "http://imge.kugou.com/stdmusic/{size}/20231023/20231023173804899287.jpg",
          "album_audio_id": 52303606,
          "filesize_320": 11699974,
          "songname": "風の住む街 (风居住的街道)(Piano Ver.)",
          "album_id": 1891927,
          "audio_id": 19773857,
          "timelength": 286249,
          "extname": "mp3",
          "filesize": 4580684,
          "hash": "6756B6381CEE01E324A5EED5D2C0C04E"
        },
        "comment_total": 4,
        "g_visible_type": 0,
        "g_created_location": "贵州",
        "collection_type": "cloudlist_collection_info",
        "userid": 386862023,
        "created_at": "2023-01-21 02:45:20",
        "reviewed_at": "2023-01-21 02:46:03",
        "song_type": "kmr_v1_audio",
        "like_total": 58,
        "topic": null,
        "id": "1070052180466129481",
        "detail_url": "",
        "released_at": "2023-01-21 02:46:03",
        "updated_at": "2023-01-21 02:45:20",
        "topic_type": "scene_music_topic_detail",
        "g_not_rec": 0,
        "content_ver": 1,
        "content": "失眠的人",
        "haslike": 0,
        "is_anonymous": 0,
        "g_user": {
     
  ...(已截断，完整数据见 responses/)
```

### /scene/video/list

场景视频列表

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| tag_id | string | 是 | - | 标签ID |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ timestamp: number
  └ list: array
errcode: number
status: number
error: string
```

**返回示例**（截断）

```json
{
  "data": {
    "timestamp": 1788516888,
    "list": []
  },
  "errcode": 0,
  "status": 1,
  "error": ""
}
```

### /theme/music

主题音乐

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| ids | string | 是 | - | 主题分类ID(逗号分隔) |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ all_theme_id: string
  └ theme_list: array
  └ all_theme_detail_list: array
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "data": {
    "all_theme_id": "1002,1997,2002,1007,2003,1008",
    "theme_list": [
      {
        "all_theme_id": "1002,1997,2002,1007,2003,1008",
        "mid": "00",
        "client_playlist_flag": 0,
        "theme_detail": {
          "id": 1002,
          "type": "style",
          "title_color_hex": "#205E71",
          "pic_net_save": "http://imge.kugou.com/commendpic/20210916/20210916175146454874.png",
          "card_pic": "http://imge.kugou.com/commendpic/20211122/20211122172544605101.jpg",
          "en_title": "",
          "title": "国语流行佳作",
          "tag": "#国语 #流行",
          "sub_title": "国语潮流打卡地",
          "rec_desc": "好歌每日更新",
          "pic": "http://imge.kugou.com/commendpic/20210916/20210916175146454874.png",
          "intro": "精挑细选的分类好歌，在这里挖掘宝藏歌曲吧",
          "detail_pic": "http://imge.kugou.com/commendpic/20210927/20210927140916732703.jpg"
        },
        "song_list": [
          {
            "filesize_flac": 24001217,
            "official_songname": "",
            "ori_audio_name": "去北极忘记你",
            "hash_192": "",
            "hash_flac": "0B42108DE072EE3DAAC9E98FB0E74275",
            "bitrate_flac": 764,
            "songname": "去北极忘记你",
            "music_trac": 1,
            "fail_process": 4,
            "pay_type": 3,
            "filesize_other": 0,
            "album_id": "97360472",
            "privilege_high": "10",
            "like_count": 1,
            "remark": "",
            "language": "国语",
            "is_file_head_320": 100,
            "alg_path": "citid=;itid=",
            "is_file_head": 100,
            
  ...(已截断，完整数据见 responses/)
```

### /theme/music/detail

主题音乐详情

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | string | 是 | - | 主题分类ID |

**返回数据**

> 实测返回为业务错误（业务码 `None`），以下为上游真实错误信封；成功结构待在有效参数/环境下补充：

**返回字段结构**（基于真实返回推导）

```
data: string
```

**返回示例**（截断）

```json
{
  "data": "<html>\r\n<head><title>500 Internal Server Error</title></head>\r\n<body bgcolor=\"white\">\r\n<center><h1>500 Internal Server Error</h1></center>\r\n<hr><center>openresty</center>\r\n</body>\r\n</html>\r\n"
}
```

### /theme/playlist

主题歌单

**请求参数**：无

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ theme_list: array
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "data": {
    "theme_list": [
      {
        "id": 6,
        "type": "time_scene",
        "play_count": 0,
        "en_title": "DRIVE",
        "pic_net_save": "http://imge.kugou.com/commendpic/20210915/20210915185254430186.jpg",
        "show_tm": [
          {
            "end": "19:00",
            "beg": "17:00"
          }
        ],
        "theme_list_title": "主题歌单",
        "pic": "http://imge.kugou.com/commendpic/20210916/20210916151754550899.jpg",
        "sort_score": 10000,
        "title": "一起去兜风",
        "intro": "告别工作的枯燥，音乐陪你释放",
        "detail_pic": "http://imge.kugou.com/commendpic/20210923/20210923115006298161.jpg"
      },
      {
        "id": 18,
        "pic_net_save": "http://imge.kugou.com/commendpic/20210916/20210916103710769840.jpg",
        "title": "华语专属订制",
        "type": "language",
        "play_count": 0,
        "en_title": "CHINESE",
        "pic": "http://imge.kugou.com/commendpic/20210916/20210916152016766656.jpg",
        "intro": "华语歌曲私人个性化订制，每日更新",
        "detail_pic": "http://imge.kugou.com/commendpic/20210923/20210923115251649269.jpg"
      },
      {
        "id": 17,
        "pic_net_save": "http://imge.kugou.com/commendpic/20210916/20210916103657679304.jpg",
        "title": "网络弄潮儿",
        "type": "style",
        "play_count": 0,
        "en_title": "POPULAR",
        "pic": "http://imge.kugou.com/commendpic/20210916/20210916152006808881.jpg",
        "intro": "网络流行热歌，这里为你一网打尽",
        "detail_pic": "http://imge.kugou.com/commendpic/20210923/20210923115241756684.jpg"
      },
      {
        "id": 19,
        "pic_n
  ...(已截断，完整数据见 responses/)
```

### /theme/playlist/track

主题歌单内曲目

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| theme_id | string | 是 | - | 主题ID |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ client_playlist_flag: number
  └ mid: string
  └ collect_total: number
  └ theme_detail: object
  └ song_list: array
  └ sign: string
  └ song_list_size: number
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "data": {
    "client_playlist_flag": 0,
    "mid": "00",
    "collect_total": 365284,
    "theme_detail": {
      "id": 1,
      "pic_net_save": "http://imge.kugou.com/commendpic/20210915/20210915184619661863.jpg",
      "title": "超深度睡眠",
      "play_count": 0,
      "detail_pic": "http://imge.kugou.com/commendpic/20210923/20210923114854480809.jpg",
      "pic": "http://imge.kugou.com/commendpic/20210916/20210916151635572701.jpg",
      "en_title": "DEEP SLEEP",
      "intro": "夜深了，让音乐轻轻为你关上床头灯",
      "type": "time_scene"
    },
    "song_list": [
      {
        "mv_hash": "",
        "hash": "1FE848C07AD92EDF859BFBEC6D604F10",
        "singerinfo": [
          {
            "name": "Meditation Zen",
            "is_publish": "1",
            "id": "1125723"
          },
          {
            "name": "Música para Relaxar Maestro",
            "is_publish": "1",
            "id": "1168603"
          },
          {
            "name": "Nature Sounds Nature Music",
            "is_publish": "1",
            "id": "374066"
          }
        ],
        "tags": [
          {
            "tag_id": "955",
            "parent_id": "0",
            "tag_name": "2018"
          },
          {
            "tag_id": "1608",
            "parent_id": "960",
            "tag_name": "未细分纯音乐"
          },
          {
            "tag_id": "3290",
            "parent_id": "7",
            "tag_name": "纯音乐"
          }
        ],
        "official_songname": "",
        "bitrate": 128,
        "ori_audio_name": "Soothing Tides",
        "hash_192": "",
        "has_accompany": 0,
  
  ...(已截断，完整数据见 responses/)
```

### /top/card

顶部卡片（首页运营位）

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| card_id | number | 否 | 1 | 卡片ID |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ OlexpIds: string
  └ song_list: array
  └ song_list_size: number
  └ bi_biz: string
  └ rec_desc: string
  └ rec_user_nickname: string
  └ sign: string
  └ card_id: number
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "data": {
    "OlexpIds": "7011,7013,7031,7055,7085,7095,7099,7215,7341,7343,7345,7347,7539,7541,7797,7805,7921,7925,7927,8375,8377,8379,8381,8403,8431,8433,8471,8473,8483,8485,8841,8843,8845,8847,8897,8899,9099,9101,9231,9233,9251,9253,9311,9313,9653,9655,10261",
    "song_list": [
      {
        "filesize_flac": 24385780,
        "official_songname": "",
        "ori_audio_name": "遇上你之前的我",
        "hash_192": "",
        "is_rec": 1,
        "hash_flac": "5CA5E8562F7F96D6CBF6A0DDBFC937CC",
        "songname": "遇上你之前的我",
        "music_trac": 1,
        "fail_process": 4,
        "pay_type": 3,
        "filesize_other": 0,
        "album_id": 91363116,
        "remark": "",
        "language": "国语",
        "is_file_head_320": 100,
        "alg_path": "s=10,d=kaiyuan_recall,t=1788516871,a=recall*copr=1,rn=38008*recall_bpj=2,itid=3aa57353-d8aa-5bdd-729d-1ea52e8557f6,qsid=0,qmid=0#",
        "is_file_head": 100,
        "timelength_320": 215.98,
        "scid": 344539852,
        "suffix_audio_name": "",
        "mv_hash": "A948BCBCD1C1F79CF3C71674BE1B03E7",
        "hash": "00A8258E40F3D66EBF963291C2CA0FC3",
        "author_name": "Gareth.T",
        "tags": [
          {
            "tag_id": 84,
            "parent_id": 7,
            "tag_name": "国语"
          },
          {
            "tag_id": 882,
            "parent_id": 3,
            "tag_name": "流行"
          },
          {
            "tag_id": 988,
            "parent_id": 882,
            "tag_name": "网络流行"
          },
          {
            "tag_id": 1534,
            "parent_id": 988,
            "ta
  ...(已截断，完整数据见 responses/)
```

### /top/card/youth

青年模式顶部卡片

> 实测上游请求 URL 拼接异常（`https://gateway.kugou.comyouth/...`，疑似 host+router 拼接问题），待排查。

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| tagid | string | 否 | - | 标签ID |
| card_id | number | 否 | 3005 | 卡片ID |
| pagesize | number | 否 | 30 | 每页数量 |

**返回数据**

> 实测返回为业务错误（业务码 `0`），以下为上游真实错误信封；成功结构待在有效参数/环境下补充：

**返回字段结构**（基于真实返回推导）

```
status: number
msg: string
```

**返回示例**（截断）

```json
{
  "status": 0,
  "msg": "error sending request for url (https://gateway.kugou.comyouth/v1/song/single_card_recommend?dfid=-&mid=66759400758721805038086495813949956634&uuid=-&appid=1005&clientver=20489&clienttime=1788517581&card_id=3005&area_code=1&platform=ops&module_id=1&ver=v2&pagesize=30&signature=32f2c78594e8dcd1b478be1eee593442)"
}
```

### /yueku

乐库内容

**请求参数**：无

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ timestamp: number
  └ info: object
errcode: number
status: number
error: string
```

**返回示例**（截断）

```json
{
  "data": {
    "timestamp": 1788516889,
    "info": {
      "song": [
        {
          "320hash": "88CBCD445BB27D9591E25EE67F844D2C",
          "hash": "14ECD0A2A68CD70111C773430BEFBEF7",
          "sqfilesize": 27836360,
          "sqprivilege": 10,
          "bitrate": 128,
          "singername": "周深",
          "320filesize": 9671771,
          "feetype": 0,
          "songname": "我们不散",
          "filename": "周深 - 我们不散",
          "trans_param": {
            "init_pub_day": 3,
            "ogg_128_hash": "1C5AFD34EF0A3062EB219D0811D96071",
            "classmap": {
              "attr0": 234881032
            },
            "language": "国语",
            "free_limited": 1,
            "cpy_attr0": 25174048,
            "musicpack_advance": 0,
            "display": 0,
            "display_rate": 0,
            "cpy_map": "2",
            "cpy_level": 1,
            "cpy_grade": 5,
            "qualitymap": {
              "bits": "800000db41080003d00c074",
              "attr0": 1023459444,
              "attr1": 1746993152
            },
            "is_original": 1,
            "pay_block_tpl": 1,
            "cid": 658997051,
            "hash_multitrack": "95393BC336036E15373E94EF26F425E9",
            "ipmap": {
              "attr0": 138546253824
            },
            "ogg_320_hash": "2637E49EF05B794D584F326D7DCC21F6",
            "ogg_128_filesize": 2911294,
            "union_cover": "http://imge.kugou.com/stdmusic/{size}/20260831/20260831191511251408.jpg",
            "provider": 2,
            "ogg_320_filesize": 10181520
          },
          "Ac
  ...(已截断，完整数据见 responses/)
```

### /yueku/banner

乐库 Banner

**请求参数**：无

**返回数据**

> 实测返回为业务错误（业务码 `0`），以下为上游真实错误信封；成功结构待在有效参数/环境下补充：

**返回字段结构**（基于真实返回推导）

```
status: number
error_code: number
data: object
  └ timestamp: number
  └ ads: array
```

**返回示例**（截断）

```json
{
  "status": 0,
  "error_code": 31136,
  "data": {
    "timestamp": 1788517320,
    "ads": []
  }
}
```

### /yueku/fm

乐库电台

**请求参数**：无

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: array  // array<object>，元素字段见下
  └ [].time_fm_cn: string
  └ [].rcm_text: string
  └ [].fm_list: array
msg: string
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "data": [
    {
      "time_fm_cn": "傍晚",
      "rcm_text": "窗外是和煦的夕阳",
      "fm_list": [
        {
          "imgUrl24": "http://imge.kugou.com/fmlogo/24/20211228155014909894.jpg",
          "fmtype": 2,
          "fm_id": 342,
          "imgUrl100": "http://imge.kugou.com/fmlogo/145/20211228155014187651.jpg",
          "imgUrl32": "http://imge.kugou.com/fmlogo/32/20211228155014349786.jpg",
          "song_info": {
            "hash": "66AA4B6D176FDA153DFEE1C580F918FD",
            "hash_super": "",
            "hmvhash": "",
            "m4ahash": "",
            "topic_remark": "",
            "audio_id": 389517887,
            "m4asize": 0,
            "price_128": 0,
            "topic_url": "",
            "320size": 8622802,
            "bitrate": 128,
            "type_320": "",
            "musictrac": 0,
            "privilege_320": 0,
            "pay_type_320": 0,
            "size": 3449303,
            "pay_type_128": 0,
            "hashflac": "6A52556505237DCBA63DDD99B29BA1DD",
            "album_audio_id": 677212480,
            "name": "张杰、HOYO-MiX - 经过",
            "fail_process": 0,
            "pay_type": 0,
            "pkg_price_flac": 0,
            "pay_type_flac": 0,
            "album_id": 104694877,
            "ext": "mp3",
            "filesize_super": 0,
            "price_flac": 0,
            "mvhash": "",
            "type": "",
            "filesize_flac": 24505141,
            "320hash": "0A6ECCA3DCF9B33D042C61401BE45FF7",
            "isfilehead": 100,
            "trac": 0,
            "hash_ape": "",
            "privilege_flac":
  ...(已截断，完整数据见 responses/)
```

## 登录与设备

### /captcha/sent

发送短信验证码

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| mobile | string | 是 | - | 手机号 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ count: number
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "data": {
    "count": 8
  },
  "status": 1,
  "error_code": 0
}
```

### /login

登录（账号相关聚合入口）

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| username | string | 否 | - | 用户名 |
| password | string | 是 | - | 密码 |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /login/cellphone

手机号登录

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| mobile | string | 是 | - | 手机号 |
| code | string | 是 | - | 验证码 |
| userid | string | 否 | - | 用户ID |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /login/device

登录设备列表

**请求参数**：无

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /login/device/kick

踢下线设备

**请求参数**：无

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /login/openplat

第三方开放平台登录

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| code | string | 是 | - | 微信授权code |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /login/qr/check

二维码登录状态检测

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| key | string | 是 | - | 二维码key |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ status: number
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "data": {
    "status": 0
  },
  "status": 1,
  "error_code": 0
}
```

### /login/qr/create

二维码登录生成二维码

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| key | string | 是 | - | 二维码key |
| qrimg | boolean | 否 | false | 是否返回二维码图片base64 |

**返回数据**

> 实测返回为业务错误（业务码 `None`），以下为上游真实错误信封；成功结构待在有效参数/环境下补充：

**返回字段结构**（基于真实返回推导）

```
code: number
data: object
  └ url: string
  └ base64: string
```

**返回示例**（截断）

```json
{
  "code": 200,
  "data": {
    "url": "https://h5.kugou.com/apps/loginQRCode/html/index.html?qrcode=e291fc24c9dbe4ef3fdd9837ea3e1a9c",
    "base64": ""
  }
}
```

### /login/qr/key

二维码登录 key

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| type | string | 否 | android | 类型 web 为网页端 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ qrcode: string
  └ qrcode_img: string
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "data": {
    "qrcode": "7b85ebbe3d61718a19f1cfbcdc32f2821014",
    "qrcode_img": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAacAAAGnAQMAAAApIQrbAAAABlBMVEUAAAD///+l2Z/dAAAAAnRSTlP//8i138cAAAAJcEhZcwAACxIAAAsSAdLdfvwAAAOgSURBVHic7Zw9bsMwDIUZdMjoI/goPppztB7FR8joIQgr8j1SblF06EaAmlRBn5dQj39SRf8zpKli1EMwVD9F7ra0qz5jZTk/3viyTTA+mqpHbbb2tskx9tgfY7O+bupb8jt3TbypgtQxfu438Zf45GmbfeWU25jICmoY0tJUbWpMxExiGInZBsTBrMUo7GmqOKV6+J5BQed9cn9JyHtTlSnqvPtubFj1pElggvGrd2iqBsWfnt78/deERtJUOSqHSYGdd5s/Ke9+3l35IfgcTVWjZmTuv7tPVur8UACKw4pwjjlaU+UoMwALv4c3XyIRQ9olPklHPxRg16OpipSddz/4OO80EuZfR37QxUH2sI2malHIraxgMo+5HXyzDcEEWTYzskWbqkeNPcigbfMZWTYmIpluLyyzhAI0VY7y826eGqHaptNIPCDf7csuBZ/MspuqRomPTMRcCpBuZ/6VLsCKadpUOepxidlkGkBGcS+uWDhn3rypkhTD74jHZvXbrAMTOHrgkbU1VYlynXeXjdN9aW1oSEHq/AjedHqHpspQSLIkbGOm0tHAwkTjg0tTZSk/7wjR3RJcCnZ2PXDwIxNXbaoeJRGhCWxD4dZtbEc48Yzi3ESaqkbNtMuXb8i/sl+pKJSlFAg7WU2VoixmixsmaEoOnY9si3jWW7KW0lQ5ajlnCYXh9+xXikiEaj+9eVNlKP/dfVwTMa6oRgllrqg2VY7CiqfSImx2iGTwFpsZxbk4NFWOUqr6tQcNKchEDCE6lf+JWkpT1ajMqdHJMgPwgDwyMvZBWBcN22iqEuUVs+hFujfXuKsvsdkXQ/CbqkhR3oW2kXn3pC5llrx70FQtao2XU8cSF//iNu8aj+nylmBTJalHVra37E5ebhaxmHb9clMVKcmHGPmEKq9zs2Lmkxm9N1WPgm2w1s04XNnSyonwmZWbTVPVKBvxpmaJyFwkelt3igMT8FCApopRONTxgF2vFz43ZfBmbyfp37WpglSsXAukOiulP0pnItHJaqoSlcOrYYzZ9NL1iO9873E0VYp68Bc38Mn/YKARvK1xeTvCuS36lU3VoswAFAoAVbdKKSxhQ979692DpmpRB+4e2JhXDmAw63wGK7FyNlWaQvWbtW6dBVIvqmAlqt9NVabY0WAcPoxE079PF9BUQcp+9DeK3qyPHXmve76UZPsjbwU3VYqigGdX2lUd//lt03knMM3m3lQ96j+jqVrUFyGyFm7JOppyAAAAAElFTkSuQmCC"
  },
  "status": 1,
  "error_code": 0
}
```

### /login/token

token 登录（换票）

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| token | string | 否 | - | 登录token |
| userid | string | 否 | 0 | 用户ID |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /login/wx/check

微信登录状态检测

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| uuid | string | 是 | - | 扫码uuid |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /login/wx/create

微信登录二维码

**请求参数**：无

**返回数据**

> 实测返回为业务错误（业务码 `0`），以下为上游真实错误信封；成功结构待在有效参数/环境下补充：

**返回字段结构**（基于真实返回推导）

```
errcode: number
uuid: string
appname: string
qrcode: object
  └ qrcodebase64: string
  └ qrcodelength: number
  └ qrcodeurl: string
```

**返回示例**（截断）

```json
{
  "errcode": 0,
  "uuid": "041U2Jhx3VpHFa1j",
  "appname": "酷狗音乐",
  "qrcode": {
    "qrcodebase64": "/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAMCAgMCAgMDAwMEAwMEBQgFBQQEBQoHBwYIDAoMDAsKCwsNDhIQDQ4RDgsLEBYQERMUFRUVDA8XGBYUGBIUFRT/wAALCAHWAdYBAREA/8QAHwAAAQUBAQEBAQEAAAAAAAAAAAECAwQFBgcICQoL/8QAtRAAAgEDAwIEAwUFBAQAAAF9AQIDAAQRBRIhMUEGE1FhByJxFDKBkaEII0KxwRVS0fAkM2JyggkKFhcYGRolJicoKSo0NTY3ODk6Q0RFRkdISUpTVFVWV1hZWmNkZWZnaGlqc3R1dnd4eXqDhIWGh4iJipKTlJWWl5iZmqKjpKWmp6ipqrKztLW2t7i5usLDxMXGx8jJytLT1NXW19jZ2uHi4+Tl5ufo6erx8vP09fb3+Pn6/9oACAEBAAA/AP1TooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooorlPil8UvDHwW8Can4y8Zan/Y3hvTfK+13v2eWfy/MlSJPkiVnOXkQcKcZyeATXgH/AA9H/Zi/6Kb/AOUDVP8A5Go/4ej/ALMX/RTf/KBqn/yNR/w9H/Zi/wCim/8AlA1T/wCRqP8Ah6P+zF/0U3/ygap/8jUf8PR/2Yv+im/+UDVP/kaj/h6P+zF/0U3/AMoGqf8AyNR/w9H/AGYv+im/+UDVP/kaj/h6P+zF/wBFN/8AKBqn/wAjV6r8C/2o/hh+0p/bf/CuPE3/AAkf9i+R9v8A9AurXyfO8zyv9fEm7PlSfdzjbzjIz1XxS+KXhj4LeBNT8ZeMtT/sbw3pvlfa737PLP5fmSpEnyRKznLyIOFOM5PAJrwD/h6P+zF/0U3/AMoGqf8AyNR/w9H/AGYv+im/+UDVP/kaj/h6P+zF/wBFN/8AKBqn/wAjUf8AD0f9mL/opv8A5QNU/wDkaj/h6P8Asxf9FN/8oGqf/I1H/D0f9mL/AKKb/wCUDVP/AJGo/wCHo/7MX/RTf/KBqn/yNR/w9H/Zi/6Kb/5QNU/+RqP+Ho/7MX/RTf8Aygap/wDI1dV8Lf29fgT8afHemeDfBvjn+2fEmpeb9ksv7Iv4PM8uJ5X+eWBUGEjc8sM4wOSBX0BXyr/w9H/Zi/6Kb/5QNU/+RqP+Ho/7MX/RTf8Aygap/wDI1H/D0f8AZi/6Kb/5QNU/+RqP+Ho/7MX/AEU3/wAoGqf/ACNR/wAPR/2Yv+im/wDlA1T/AORqP+Ho/wCzF/0U3/ygap/8jV7/APC34peGPjT4E0zxl4N1P+2fDepeb9kvfs8sHmeXK8T/ACSqrjDxuOVGcZHBBrlfjp+1H8MP2a/7E/4WP4m/4Rz+2vP+
  ...(已截断，完整数据见 responses/)
```

### /register/dev

设备注册

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| availableRamSize | number | 否 | 4983533568 | 可用内存 |
| availableRomSize | number | 否 | 48114719 | 可用存储 |
| availableSDSize | number | 否 | 48114717 | 可用SD |
| basebandVer | string | 否 | - | 基带版本 |
| batteryLevel | number | 否 | 100 | 电量 |
| batteryStatus | number | 否 | 3 | 充电状态 |
| brand | string | 否 | Redmi | 品牌 |
| buildSerial | string | 否 | unknown | 序列号 |
| device | string | 否 | marble | 设备型号 |
| imei | string | 否 | - | IMEI |
| imsi | string | 否 | - | IMSI |
| manufacturer | string | 否 | Xiaomi | 厂商 |
| uuid | string | 否 | - | 设备UUID |
| accelerometer | boolean | 否 | false | 加速度传感器 |
| accelerometerValue | string | 否 | - | 加速度值 |
| gravity | boolean | 否 | false | 重力传感器 |
| gravityValue | string | 否 | - | 重力值 |
| gyroscope | boolean | 否 | false | 陀螺仪 |
| gyroscopeValue | string | 否 | - | 陀螺仪值 |
| light | boolean | 否 | false | 光线传感器 |
| lightValue | string | 否 | - | 光线值 |
| magnetic | boolean | 否 | false | 磁力传感器 |
| magneticValue | string | 否 | - | 磁力值 |
| orientation | boolean | 否 | false | 方向传感器 |
| orientationValue | string | 否 | - | 方向值 |
| pressure | boolean | 否 | false | 压力传感器 |
| pressureValue | string | 否 | - | 压力值 |
| step_counter | boolean | 否 | false | 计步传感器 |
| step_counterValue | string | 否 | - | 计步值 |
| temperature | boolean | 否 | false | 温度传感器 |
| temperatureValue | string | 否 | - | 温度值 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ dfid: string
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "data": {
    "dfid": "3497jA4HOWy22xqhSx3XC3di"
  },
  "status": 1,
  "error_code": 0
}
```

## 用户

### /user/cloud

用户云盘歌曲列表

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /user/cloud/url

用户云盘歌曲播放地址

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| hash | string | 是 | - | 歌曲hash |
| album_audio_id | number | 否 | 0 | 音频ID |
| audio_id | number | 否 | 0 | 歌曲ID |
| name | string | 否 | - | 歌曲名 |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /user/detail

用户详情

**请求参数**：无

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /user/follow

关注用户

**请求参数**：无

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /user/follow/message

用户关注消息

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | string | 是 | - | 消息ID |
| pagesize | number | 否 | 30 | 每页数量 |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /user/history

用户历史（听歌/播放）

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| bp | string | 否 | - | 业务参数 |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /user/listen

用户在听

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| type | number | 否 | 0 | 列表类型 |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /user/playlist

用户歌单

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /user/video/collect

用户收藏的视频

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /user/video/love

用户喜欢的视频

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| pagesize | number | 否 | 30 | 每页数量 |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /user/vip/detail

用户 VIP 详情

**请求参数**：无

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

## 视频

### /video/detail

视频详情

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | string | 是 | - | 视频ID(逗号分隔) |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
status: number
error_code: number
errcode: number
errmsg: string
data: array
```

**返回示例**（截断）

```json
{
  "status": 1,
  "error_code": 0,
  "errcode": 0,
  "errmsg": "",
  "data": [
    {}
  ]
}
```

### /video/privilege

视频权限

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| hash | string | 是 | - | 视频hash(逗号分隔) |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: array  // array<object>，元素字段见下
  └ [].hash: string
  └ [].level: number
  └ [].status: number
  └ []._msg: string
  └ [].relate_goods: array
  └ [].singer_name: string
  └ [].sale_mode: number
  └ [].trans_param: array
  └ [].name: string
  └ [].fail_process: number
  └ []._errno: number
  └ [].cid: number
  └ [].info: object
  └ [].privilege: number
  └ [].id: number
  └ [].prices: object
  └ [].pay_type: number
error_code: number
status: number
message: string
```

**返回示例**（截断）

```json
{
  "data": [
    {
      "hash": "E291FC24C9DBE4EF3FDD9837EA3E1A9C",
      "level": 0,
      "status": 1,
      "_msg": "Allow: the video is not found.",
      "relate_goods": [],
      "singer_name": "",
      "sale_mode": 0,
      "trans_param": [],
      "name": "",
      "fail_process": 0,
      "_errno": 6,
      "cid": -1,
      "info": {
        "filesize": 0,
        "bitrate": 0
      },
      "privilege": 0,
      "id": 0,
      "prices": {
        "rgl": 0
      },
      "pay_type": 0
    }
  ],
  "error_code": 0,
  "status": 1,
  "message": ""
}
```

### /video/url

视频播放地址

> 需传入真实的 MV/视频 hash（歌曲 hash 无效，实测返回 Not found）。

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| hash | string | 否 | - | 视频hash |

**返回数据**

> 实测返回为业务错误（业务码 `0`），以下为上游真实错误信封；成功结构待在有效参数/环境下补充：

**返回字段结构**（基于真实返回推导）

```
privileges: object
  └ 4f18553f2af06e355ad75a5ee6173456: number
errcode: number
status: number
error: string
```

**返回示例**（截断）

```json
{
  "privileges": {
    "4f18553f2af06e355ad75a5ee6173456": 0
  },
  "errcode": 40002,
  "status": 0,
  "error": "Not found"
}
```

## 长音频

### /longaudio/album/audios

长音频专辑内音频列表

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| album_id | string | 是 | - | 长音频专辑ID |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
total: number
error_code: number
data: array
extra: object
  └ reverse: number
status: number
errmsg: string
```

**返回示例**（截断）

```json
{
  "total": 0,
  "error_code": 0,
  "data": [],
  "extra": {
    "reverse": 1
  },
  "status": 1,
  "errmsg": ""
}
```

### /longaudio/album/detail

长音频专辑详情

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| album_id | string | 是 | - | 长音频专辑ID，逗号分隔 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
status: number
error_code: number
errcode: number
errmsg: string
data: array  // array<object>，元素字段见下
  └ [].album_name: string
  └ [].author_name: string
  └ [].trans_param: object
  └ [].album_tag: array
  └ [].play_times: number
  └ [].full_intro: string
  └ [].mix_intro: string
  └ [].album_id: string
  └ [].cover: string
  └ [].authors: array
  └ [].sizable_cover: string
  └ [].intro: string
  └ [].category: string
  └ [].language: string
  └ [].publish_company: string
```

**返回示例**（截断）

```json
{
  "status": 1,
  "error_code": 0,
  "errcode": 0,
  "errmsg": "",
  "data": [
    {
      "album_name": "",
      "author_name": "",
      "trans_param": {},
      "album_tag": [],
      "play_times": 0,
      "full_intro": "",
      "mix_intro": "",
      "album_id": "",
      "cover": "",
      "authors": [],
      "sizable_cover": "",
      "intro": "",
      "category": "",
      "language": "",
      "publish_company": ""
    }
  ]
}
```

### /longaudio/daily/recommend

长音频每日推荐

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ is_end: number
  └ albums: array
  └ current_time: number
errcode: number
status: number
errmsg: string
```

**返回示例**（截断）

```json
{
  "data": {
    "is_end": 0,
    "albums": [
      {
        "rank_tag_desc": "",
        "sizable_cover": "https://imgessl.kugou.com/stdmusic/{size}/20240302/20240302083303887607.jpg",
        "tag_info": {
          "tag_name": "自然疗愈",
          "tag_id": 3311
        },
        "alg_path": "recall-hot",
        "special_tag": 19968,
        "album_name": "催眠下雨声，让你快速进入深度睡眠",
        "album_label": {
          "label_name": "",
          "label_id": 0
        },
        "recommend_type": 2,
        "sizable_cover_h5": "https://imgessl.kugou.com/stdmusic/240/20240302/20240302083303887607.jpg",
        "audio_total": 121,
        "rec_tag": {
          "album_id": 88314495,
          "type": 1,
          "tag_info": {
            "album_id": "88314495",
            "tag_id": 3311,
            "p_tag_id": 1070,
            "tag_name": "自然疗愈"
          },
          "rank_info": {
            "rank_tag_id": 1070,
            "rank_id": 1
          },
          "desc": "助眠解压免费 NO.33",
          "cat_desc": "自然疗愈"
        },
        "recommend_reason": "根据大家的口味推荐",
        "publish_date": "2024-03-02",
        "album_id": 88314495,
        "source_type": 1,
        "is_pay": 0,
        "play_count": 65832206,
        "intro": "听着下雨的声音，快速进入深度睡眠。",
        "subscribe_desc": ""
      },
      {
        "rank_tag_desc": "",
        "sizable_cover": "https://imgessl.kugou.com/stdmusic/{size}/20240218/20240218141502883779.jpg",
        "tag_info": {
          "tag_name": "搞笑段子",
          "tag_id": 1108
        },
        "alg_path": "recall-hot",
        "special_tag": 3584,
       
  ...(已截断，完整数据见 responses/)
```

### /longaudio/rank/recommend

长音频榜单推荐

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| platform | string | 否 | ios | 平台 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: array  // array<object>，元素字段见下
  └ [].source_type: number
  └ [].rank_id: number
  └ [].tag_id: number
  └ [].albums: array
  └ [].rank_name: string
errcode: number
status: number
errmsg: string
```

**返回示例**（截断）

```json
{
  "data": [
    {
      "source_type": 2,
      "rank_id": 1,
      "tag_id": 906,
      "albums": [
        {
          "album_name": "七零一家人：乔家小院|年代治愈相伴一生",
          "author_name": "凤鸣工作室、清雨_凤鸣",
          "sizable_cover": "https://imgessl.kugou.com/stdmusic/240/20260724/20260724155520588370.jpg",
          "play_count": 2398105,
          "publish_date": "2026-07-23",
          "album_id": 200696803,
          "audio_total": 658,
          "special_tag": 3602,
          "intro": "睁开眼，却发现自己回到了七十年代。这是一个物资匮乏、思想保守的时代，重男轻女的观念更是根深蒂固。面对着与前世相似的困境，乔小六的心中却燃起了熊熊火焰。这一次，她绝不会再任人摆布！她要用自己的智慧和勇气，打破时代的枷锁，改变自己和家人的命运。",
          "sizable_cover_source": "http://imge.kugou.com/stdmusic/{size}/20260724/20260724155520588370.jpg"
        },
        {
          "album_name": "风水有术|驭房有术|关彦之|多播剧|风水秘术玄学|悬疑灵异",
          "author_name": "关彦之",
          "sizable_cover": "https://imgessl.kugou.com/stdmusic/240/20241204/20241204173550181994.jpg",
          "play_count": 32090951,
          "publish_date": "2024-08-26",
          "album_id": 102564903,
          "audio_total": 7473,
          "special_tag": 3602,
          "intro": "进城闯荡的小阿姨衣锦还乡，张禹的老妈心动了，决定让儿子前去投奔。不曾想，所谓的豪宅就是一个三十平米的出租屋，更为要命的是，小阿姨经营的房产中介都快交不上房租了。卖房、装修……张禹从乡下棺材铺王老头那里学来的奇门玄术竟然派上了用场，摇身一变成了王牌经纪人……\n兄弟、美女，买房吗？阳宅都有，包装修！",
          "sizable_cover_source": "http://imge.kugou.com/stdmusic/{size}/20241204/20241204173550181994.jpg"
        },
        {
          "album_name": "相错亲结对婚，亿万老公太粘人",
          "author_name": "酷匠听书",
          "sizable_cover": "https://imgessl.kugou.com/stdmusic/240/20260305/20260305094520496451.jpg",
          "play_co
  ...(已截断，完整数据见 responses/)
```

### /longaudio/vip/recommend

长音频 VIP 推荐

**请求参数**：无

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ list: array
  └ is_end: number
  └ title: string
errcode: number
status: number
errmsg: string
```

**返回示例**（截断）

```json
{
  "data": {
    "list": [
      {
        "album_name": "道士不好惹|全新重制多人版|古井观传奇|风水玄学|骨头演播",
        "album_id": 103182781,
        "source_type": 2,
        "sizable_cover": "https://imgessl.kugou.com/stdmusic/{size}/20251113/20251113185905655752.jpg",
        "special_tag": 2049,
        "play_count": 170964028
      },
      {
        "album_name": "虹猫蓝兔七侠传|官方正版动画原声精品有声剧",
        "album_id": 205858708,
        "source_type": 2,
        "sizable_cover": "https://imgessl.kugou.com/stdmusic/{size}/20260826/20260826142931669259.jpg",
        "special_tag": 4197893,
        "play_count": 3690
      },
      {
        "album_name": "史上最强赘婿|多人剧|轻松爆笑无厘头|赘婿|曲中人工作室",
        "album_id": 193672050,
        "source_type": 2,
        "sizable_cover": "https://imgessl.kugou.com/stdmusic/{size}/20260630/20260630202722334545.jpg",
        "special_tag": 3585,
        "play_count": 1874599
      },
      {
        "album_name": "龙小腾的神兽冒险|山海经·神话传说·睡前|宝宝巴士",
        "album_id": 206056100,
        "source_type": 2,
        "sizable_cover": "https://imgessl.kugou.com/stdmusic/{size}/20260901/20260901141821543230.jpg",
        "special_tag": 2051,
        "play_count": 833
      },
      {
        "album_name": "战王殿下，王妃替您出征了",
        "album_id": 68382638,
        "source_type": 2,
        "sizable_cover": "https://imgessl.kugou.com/stdmusic/{size}/20230213/20230213013005749445.jpg",
        "special_tag": 3587,
        "play_count": 690010
      },
      {
        "album_name": "小猪佩奇第12季中文正版动画原声",
        "album_id": 203826660,
        "source_type": 2,
        "sizable_cover": "https://imges
  ...(已截断，完整数据见 responses/)
```

### /longaudio/week/recommend

长音频每周推荐

**请求参数**：无

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ list: array
  └ title: string
errcode: number
status: number
errmsg: string
```

**返回示例**（截断）

```json
{
  "data": {
    "list": [
      {
        "album_name": "史上最强赘婿|百人多播剧|轻松搞笑|小白爽文|软饭流",
        "album_id": 130348979,
        "source_type": 2,
        "sizable_cover": "https://imgessl.kugou.com/stdmusic/{size}/20241126/20241126142914689781.jpg",
        "special_tag": 3587,
        "recommend_name": "史上最强赘婿丨百人多播有声剧丨精品重置丨玄幻丨赘婿爽文",
        "play_count": 72125969
      },
      {
        "album_name": "重生后，我成了奸臣黑月光|完本畅听|轻松搞笑|多人有声|偏方方",
        "album_id": 108326068,
        "source_type": 2,
        "sizable_cover": "https://imgessl.kugou.com/stdmusic/{size}/20241216/20241216220733237702.jpg",
        "special_tag": 3587,
        "recommend_name": "重生后，我成了奸臣黑月光|轻松搞笑|多人有声剧|偏方方新作",
        "play_count": 16260112
      },
      {
        "album_name": "金丹是恒星，你管这叫修仙|玄幻逗比搞笑|游戏也可修行多人有声剧",
        "album_id": 109033619,
        "source_type": 2,
        "sizable_cover": "https://imgessl.kugou.com/stdmusic/{size}/20241120/20241120152802823978.jpg",
        "special_tag": 3587,
        "recommend_name": "金丹是恒星，你管这叫修仙？",
        "play_count": 942731
      },
      {
        "album_name": "种田养崽:恶毒女配被全家争着宠|爆笑穿书|多人精品剧",
        "album_id": 108884762,
        "source_type": 2,
        "sizable_cover": "https://imgessl.kugou.com/stdmusic/{size}/20241118/20241118174101653079.jpg",
        "special_tag": 3587,
        "recommend_name": "种田养崽:恶毒女配被全家争着宠",
        "play_count": 1507476
      },
      {
        "album_name": "我有一身被动技|精彩多人|轻松搞笑|玄幻爽文|VIP免费",
        "album_id": 107792360,
        "source_type": 2,
        "sizable_cover": "https://imgessl.kugou.com/stdmusic/{size}/20241104/20241
  ...(已截断，完整数据见 responses/)
```

## 青年模式（youth）

### /youth/channel/all

青年频道列表

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /youth/channel/amway

青年频道安利内容

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| global_collection_id | string | 是 | - | 合集ID |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
error_msg: string
data: object
  └ channel_tab_name: string
  └ has_draft: number
  └ area_list: array
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "error_msg": "",
  "data": {
    "channel_tab_name": "",
    "has_draft": 0,
    "area_list": []
  },
  "status": 1,
  "error_code": 0
}
```

### /youth/channel/detail

青年频道详情

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| global_collection_id | string | 是 | - | 合集ID(逗号分隔) |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
error_msg: string
error_code: number
timestamp: number
activity_status: number
status: number
data: array  // array<object>，元素字段见下
  └ [].audit: number
  └ [].global_collection_id: string
```

**返回示例**（截断）

```json
{
  "error_msg": "",
  "error_code": 0,
  "timestamp": 1788516896,
  "activity_status": 1,
  "status": 1,
  "data": [
    {
      "audit": 3,
      "global_collection_id": "2550343002"
    }
  ]
}
```

### /youth/channel/similar

相似频道推荐

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| channel_id | string | 是 | - | 频道ID |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
error_msg: string
data: object
  └ list: array
  └ total: number
  └ bi_index: number
  └ has_friend_channel: number
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "error_msg": "",
  "data": {
    "list": [
      {
        "is_ai": 0,
        "last_update": 1638855789,
        "userid": 1730365278,
        "channel_avatar": "https://youthimgbssdl.kugou.com/1200a3ee3934087355feff434ef22bf9.jpg",
        "create_audit": 0,
        "name": "DJ",
        "DEL": 0,
        "song_count": 1066,
        "publish_date": 1638706558,
        "last_audit": 1,
        "real_audit": 1,
        "channel_type": 1,
        "intro": "DJ爱好者聚集地",
        "manual_audit": 1,
        "amway_update_timestamp": 1638930921,
        "show_room": 1,
        "channel_cover": "https://youthimgbssdl.kugou.com/1200a3ee3934087355feff434ef22bf9.jpg",
        "posts_update_time": 1788513404,
        "is_show": 1,
        "source_rec": "h:score=3.24567&sorter=youth_pindao_xuanqun_sorter&dt=2026-09-03&model_score=0.324567;c:actionI2i;s:youthchannel_channel_20770;t:1788516897",
        "source": 2,
        "dress_num": 52,
        "subscribe_count": 92023,
        "subscribe_status": 0,
        "is_deleted": 0,
        "topic_num": 0,
        "list_ver": 1,
        "fid_count": 1066,
        "notice_update_time": 946656000,
        "comment_count": 0,
        "online_count": 0,
        "pic": "https://youthimgbssdl.kugou.com/1200a3ee3934087355feff434ef22bf9.jpg",
        "audit": 1,
        "auto_audit": 1,
        "notice": "",
        "global_collection_id": "11638706558",
        "dig_update_time": 0,
        "is_vacant": 0
      },
      {
        "is_ai": 0,
        "last_update": 1654101701,
        "userid": 1635417533,
        "channel_avatar": "https://youthi
  ...(已截断，完整数据见 responses/)
```

### /youth/channel/song

频道内歌曲

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| global_collection_id | string | 是 | - | 合集ID |
| pagesize | number | 否 | 30 | 每页数量 |
| page | number | 否 | 1 | 页码 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
error_msg: string
data: object
  └ timestamp: number
  └ is_end: number
  └ audit: number
  └ last_fileid: number
  └ global_collection_id: string
  └ list_ver: number
  └ song_count: number
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "error_msg": "",
  "data": {
    "timestamp": 1788516897,
    "is_end": 1,
    "audit": 3,
    "last_fileid": 0,
    "global_collection_id": "2550343002",
    "list_ver": 0,
    "song_count": 0
  },
  "status": 1,
  "error_code": 0
}
```

### /youth/channel/song/detail

频道歌曲详情

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| global_collection_id | string | 是 | - | 合集ID |
| fileid | string | 是 | - | 文件ID |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
error_msg: string
data: object
  └ hash: string
  └ audio_id: number
  └ vedio: string
  └ dig_user_by: array
  └ sd_hash: string
  └ userid: number
  └ comment_list: array
  └ fileid: string
  └ topic_info: array
  └ last_rec_info: string
  └ dig_source: number
  └ quality_level: number
  └ DEL: number
  └ dest_channel_id: string
  └ rec_info: string
  └ like_status: number
  └ channel_name: string
  └ audit: number
  └ real_audit: number
  └ nick_name: string
  └ dig_user_list: array
  └ like_count: number
  └ user_pic: string
  └ remark: number
  └ video_time: number
  └ source: number
  └ gender: number
  └ dig_type: number
  └ deny_reason: number
  └ ip_area: string
  └ channel_owner: number
  └ channel_pic: string
  └ ai_song_info: array
  └ is_ai: number
  └ create_time: number
  └ cover_pic: string
  └ subscribe_count: number
  └ qhd_hash: string
  └ pic_size: array
  └ video_convert_hash: string
  └ like_user_pic: array
  └ ADU: number
  └ update_time: number
  └ pic_type: number
  └ comment_count: number
  └ use_count: number
  └ pic: string
  └ focus: array
  └ auto_audit: number
  └ mixsongid: number
  └ global_collection_id: string
  └ special_id: string
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "error_msg": "",
  "data": {
    "hash": "",
    "audio_id": 0,
    "vedio": "",
    "dig_user_by": [],
    "sd_hash": "",
    "userid": 0,
    "comment_list": [],
    "fileid": "1",
    "topic_info": [],
    "last_rec_info": "",
    "dig_source": 1,
    "quality_level": 0,
    "DEL": 0,
    "dest_channel_id": "",
    "rec_info": "",
    "like_status": 0,
    "channel_name": "",
    "audit": 0,
    "real_audit": 0,
    "nick_name": "",
    "dig_user_list": [],
    "like_count": 0,
    "user_pic": "",
    "remark": 0,
    "video_time": 0,
    "source": 0,
    "gender": 2,
    "dig_type": 0,
    "deny_reason": 0,
    "ip_area": "",
    "channel_owner": 0,
    "channel_pic": "",
    "ai_song_info": [],
    "is_ai": 0,
    "create_time": -28800,
    "cover_pic": "",
    "subscribe_count": 0,
    "qhd_hash": "",
    "pic_size": [],
    "video_convert_hash": "",
    "like_user_pic": [],
    "ADU": 0,
    "update_time": -28800,
    "pic_type": 0,
    "comment_count": 0,
    "use_count": 0,
    "pic": "",
    "focus": [],
    "auto_audit": 0,
    "mixsongid": 0,
    "global_collection_id": "2550343002",
    "special_id": ""
  },
  "status": 1,
  "error_code": 0
}
```

### /youth/channel/sub

订阅/退订频道

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| global_collection_id | string | 是 | - | 合集ID |
| t | number | 否 | 1 | 1订阅 0取消订阅 |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /youth/day/vip

青春每日 VIP 信息

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| receive_day | string | 是 | - | 领取天数标识 |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /youth/day/vip/upgrade

青春每日 VIP 升级

**请求参数**：无

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /youth/dynamic

青年动态

**请求参数**：无

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /youth/dynamic/recent

青年最近动态

**请求参数**：无

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /youth/listen/song

青年模式在听歌曲

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| mixsongid | number | 否 | 666075191 | 歌曲mixsongid |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /youth/month/vip/record

青春月 VIP 记录

**请求参数**：无

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /youth/union/vip

青春联合 VIP

**请求参数**：无

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /youth/user/song

青年用户歌曲

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| userid | string | 是 | - | 目标用户ID |
| type | number | 否 | 0 | 类型 |
| pagesize | number | 否 | 30 | 每页数量 |
| page | number | 否 | 1 | 页码 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
error_msg: string
data: object
  └ list: array
  └ total_ver: number
  └ nick_name: string
  └ userid: number
  └ is_end: number
  └ list_count: number
  └ user_pic: string
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "error_msg": "",
  "data": {
    "list": [],
    "total_ver": 0,
    "nick_name": "hbwxgjh3671",
    "userid": 309,
    "is_end": 1,
    "list_count": 0,
    "user_pic": "http://imge.kugou.com/kugouicon/165/20160113/20160113180717663307.jpg"
  },
  "status": 1,
  "error_code": 0
}
```

### /youth/vip

青春 VIP 信息

**请求参数**：无

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

## 组队活动

### /team/join

加入组队

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| team_code | string | 是 | - | 队伍码 |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /team/my

我的组队活动

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| period_id | string | 否 | - | 活动周期ID |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /team/my/info

我的组队信息

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| period_id | string | 否 | - | 活动周期ID |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /team/my/status

我的组队状态

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| period_id | string | 否 | - | 活动周期ID |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /team/period/info

组队活动期次信息

**请求参数**：无

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

## 其他

### /favorite/count

收藏数量统计

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| mixsongids | string | 是 | - | 歌曲mixsongid列表，逗号分隔 |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /ip

IP 信息

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | string | 是 | - | IP ID |
| type | string | 否 | audios | 类型 audios/albums/videos/author_list |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
status: number
error_code: number
errmsg: string
total: number
extra: object
  └ page_total: number
data: array
```

**返回示例**（截断）

```json
{
  "status": 1,
  "error_code": 0,
  "errmsg": "",
  "total": 0,
  "extra": {
    "page_total": 0
  },
  "data": []
}
```

### /ip/dateil

IP 归属地详情

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | string | 是 | - | IP ID，逗号分隔 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
status: number
error_code: number
errmsg: string
data: array
```

**返回示例**（截断）

```json
{
  "status": 1,
  "error_code": 0,
  "errmsg": "",
  "data": [
    {}
  ]
}
```

### /ip/playlist

地域歌单

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | string | 是 | - | IP ID |
| page | number | 否 | 1 | 页码 |
| pagesize | number | 否 | 30 | 每页数量 |

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ timestamp: number
  └ info: array
errcode: number
status: number
error: string
```

**返回示例**（截断）

```json
{
  "data": {
    "timestamp": 1788516851,
    "info": []
  },
  "errcode": 0,
  "status": 1,
  "error": ""
}
```

### /ip/zone

IP 地域信息

**请求参数**：无

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ list: array
errcode: number
status: number
error: string
```

**返回示例**（截断）

```json
{
  "data": {
    "list": [
      {
        "special_link": "https://miniapp.kugou.com/node/v2?type=1&id=230&path=%2Findex.html%3FisHideTitleBar%3D1%26ip_id%3D87641",
        "summary": "",
        "id": 603,
        "name": "DJ",
        "app_id": "",
        "icon": "https://imgessl.kugou.com/quickentry/20230704/20230704233204541061.jpg",
        "app_url": "",
        "resource_type": 3,
        "resource_id": "",
        "ip_id": 87641
      },
      {
        "special_link": "https://miniapp.kugou.com/node/v2?type=1&id=230&path=%2Findex.html%3FisHideTitleBar%3D1%26ip_id%3D88104",
        "summary": "",
        "id": 605,
        "name": "抖音",
        "app_id": "",
        "icon": "https://imgessl.kugou.com/quickentry/20230704/20230704233232980023.jpg",
        "app_url": "",
        "resource_type": 3,
        "resource_id": "",
        "ip_id": 88104
      },
      {
        "special_link": "https://miniapp.kugou.com/node/v2?type=1&id=230&path=%2Findex.html%3FisHideTitleBar%3D1%26ip_id%3D88226",
        "summary": "",
        "id": 601,
        "name": "经典老歌",
        "app_id": "",
        "icon": "https://imgessl.kugou.com/quickentry/20230704/20230704235426413521.jpg",
        "app_url": "",
        "resource_type": 3,
        "resource_id": "",
        "ip_id": 88226
      },
      {
        "special_link": "https://miniapp.kugou.com/node/v2?type=1&id=230&path=%2Findex.html%3FisHideTitleBar%3D1%26ip_id%3D87639",
        "summary": "",
        "id": 597,
        "name": "儿童",
        "app_id": "",
        "icon": "https://imgessl.kugou.com/quickentry/20230704/2023070
  ...(已截断，完整数据见 responses/)
```

### /ip/zone/home

地域主页

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| id | string | 否 | - | IP ID |

**返回数据**

> 实测返回为业务错误（业务码 `0`），以下为上游真实错误信封；成功结构待在有效参数/环境下补充：

**返回字段结构**（基于真实返回推导）

```
errcode: number
status: number
error: string
```

**返回示例**（截断）

```json
{
  "errcode": 0,
  "status": 0,
  "error": "请求数据异常"
}
```

### /lastest/songs/listen

最近在听歌曲

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| pagesize | number | 否 | 30 | 每页数量 |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /playhistory/upload

上报播放历史

**请求参数**

| 参数 | 类型 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- | --- |
| mxid | number | 是 | - | 歌曲mixsongid |
| time | number | 否 | 0 | 播放时间(秒,默认当前) |
| pc | number | 否 | 1 | 播放次数 |

**需登录**：是（依赖 cookie 中的 token/userid 等登录态）

**返回数据**

> 需登录后调用，返回结构暂留空，待后续拿到登录数据补充。

### /server/now

服务器时间

**请求参数**：无

**返回数据**

实测返回（业务码 `status=1`）：

**返回字段结构**（基于真实返回推导）

```
data: object
  └ timestamp: number
status: number
error_code: number
```

**返回示例**（截断）

```json
{
  "data": {
    "timestamp": 1788516805
  },
  "status": 1,
  "error_code": 0
}
```
