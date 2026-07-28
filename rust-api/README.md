# MoeKoeMusic API — Rust 版

`kugoumusicapi`（Node.js/Express 版）的 **Rust 全量移植**，基于 axum + tokio + reqwest。
与 Node 版**行为完全等价**：155 个接口、签名算法、加密逻辑、Cookie 机制、缓存策略、错误信封均逐一对照移植（包括已知 bug 的原样复刻，如 `song_url` 的 `notSign` 拼写、`cryptoAesEncrypt` 的 `opt?.key && opt?.key` 判断等）。

## 构建

```bash
cd rust
cargo build --release
# 产物: target/release/kugoumusicapi(.exe)
```

要求：Rust 1.75+（edition 2021）。Windows 下使用 rustls，无需 OpenSSL。

## 运行

```bash
# 默认端口 3000
./target/release/kugoumusicapi

# CLI 参数（与 Node 版 util/runtime.js 相同的 --key=value 语法）
./target/release/kugoumusicapi --port=3000 --platform=lite --proxy=http://127.0.0.1:7890
```

### CLI 参数 / 环境变量对照

| CLI 参数     | 环境变量          | 说明                                        |
| ------------ | ----------------- | ------------------------------------------- |
| `--port`     | `PORT`            | 监听端口，默认 3000                          |
| `--platform` | `platform`        | 平台：`android`（默认）/ `lite`             |
| `--proxy`    | `KUGOU_API_PROXY` | 上游 HTTP 代理                              |
| `--guid`     | `KUGOU_API_GUID`  | 固定设备 GUID（默认启动时随机生成）          |
| `--dev`      | `KUGOU_API_DEV`   | 设备型号                                    |
| `--mac`      | `KUGOU_API_MAC`   | MAC 地址（默认 `02:00:00:00:00:00`）        |

CLI 参数优先级高于环境变量（覆盖写入 env，与 Node 版 `applyCliOverrides` 一致）。

## 与 Node 版的等价性

| 维度         | 实现                                                                                  |
| ------------ | ------------------------------------------------------------------------------------- |
| 路由         | 155/155 全部注册（`src/modules.rs` registry），路径 = 文件名 `_`→`/`                   |
| 签名         | signature_web / android / register 三套，`serde_json` 开 `preserve_order` 保证键顺序与 `JSON.stringify` 逐字符一致 |
| 加密         | AES-CBC（CryptoJS UTF-8 key 语义）、RSA 双套（raw modpow + PKCS1-v1.5）、KRC 歌词解密   |
| Cookie       | 服务层注入 `KUGOU_API_PLATFORM/MID/GUID/DEV/MAC`，query/header/authorization 合并      |
| 缓存         | moka 复刻 apicache：key=`host+uri`，仅缓存 200，TTL 120s，命中时 `cache-control: max-age` 递减，支持 `x-apicache-bypass` / `x-apicache-force-fetch` 跳过 |
| axios 语义   | 字符串 body 不设 Content-Type（对象/数组才设 `application/json`）；仅 2xx 视为成功（validateStatus） |
| 错误信封     | 上游失败 → `502 {status:0, msg}`；模块 panic 兜底 → `{code:500}`                       |

### 对拍验证结论（2026-07-24）

- 全部 155 路由在 axum 可达（与 `module/*.js` 差集为 0）。
- 抽样 20 个接口与 Node 版并行对拍：HTTP 状态码 + 业务码（status/error_code/errcode）**全部一致**。
- 已确认 `register/dev` 上游返回 `data:[]` 或新 `dfid` 为酷狗侧非确定性（Node 版自身连续请求结果亦波动），不属实现差异。
- 缓存头行为与 apicache 一致（首次 `max-age=120`、命中递减、bypass 无缓存头）。

## 目录结构

```
rust/src/
├── main.rs      # 入口：CLI 解析 → 启动 axum
├── cli.rs       # --key=value 参数解析（对应 util/runtime.js）
├── config.rs    # ../config.json 读取、appid/clientver 选择（对应 util/config.js）
├── server.rs    # 路由分发、Cookie 注入、缓存中间件、CORS、静态文件（对应 server.js）
├── request.rs   # createRequest 移植：默认参数、签名挂载、代理、arraybuffer（对应 util/request.js）
├── crypto.rs    # AES/RSA/MD5/SHA1/KRC 解密（对应 util/crypto.js）
├── sign.rs      # 三套签名 + signParamsKey/signCloudKey（对应 util/helper.js）
├── util.rs      # 随机串、guid、mid 计算等杂项
├── jsval.rs     # JS 值语义辅助（truthy / || / ?? 等）
└── modules.rs   # 155 个接口模块 + 路由注册表（对应 module/*.js）
```

## 与 Node 版并行对拍

```bash
# 终端 1：Node 版
PORT=3110 node app.js
# 终端 2：Rust 版
PORT=3111 ./target/release/kugoumusicapi
# 对比任意接口
curl -s "http://127.0.0.1:3110/search?keywords=test" | head -c 200
curl -s "http://127.0.0.1:3111/search?keywords=test" | head -c 200
```

## License

与上游项目一致（见仓库根目录 LICENSE）。
