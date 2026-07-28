<br />
<p align="center">
    <img src="https://github.com/iAJue/MoeKoeMusic/raw/main/images/hero.png" alt="hero" width="250" height="250">
  <h2 align="center" style="font-weight: 600">MoeKoe Music</h2>
  <p align="center">
    <a href="https://github.com/MoeKoeMusic/MoeKoeMusic/releases/latest"><img src="https://img.shields.io/github/v/release/MoeKoeMusic/MoeKoeMusic?style=flat-square" /></a>
    <a href="https://github.com/MoeKoeMusic/MoeKoeMusic/stargazers"><img src="https://img.shields.io/github/stars/MoeKoeMusic/MoeKoeMusic?style=flat-square" /></a>
    <a href="https://github.com/MoeKoeMusic/MoeKoeMusic/releases"><img src="https://img.shields.io/github/downloads/MoeKoeMusic/MoeKoeMusic/total?style=flat-square" /></a>
    <a href="https://github.com/MoeKoeMusic/MoeKoeMusic/blob/main/LICENSE"><img src="https://img.shields.io/github/license/MoeKoeMusic/MoeKoeMusic?style=flat-square" /></a>
    <a href="https://github.com/iAJue"><img src="https://img.shields.io/badge/%F0%9F%8E%89_Create_by_iAJue-with_Love_%E2%9D%A4-pink?style=flat-square" /></a>
  </p>
  <p align="center">
    一款开源简洁高颜值的酷狗第三方客户端 · 基于 Tauri 2 + Rust 构建
    <br />
    <a href="https://github.com/iAJue/MoeKoeMusic/" target="blank"><strong>🌎 GitHub仓库</strong></a>&nbsp;&nbsp;|&nbsp;&nbsp;
    <a href="https://github.com/iAJue/MoeKoeMusic/releases" target="blank"><strong>📦️ 下载安装包</strong></a>&nbsp;&nbsp;|&nbsp;&nbsp;
    <a href="https://MoeJue.cn" target="blank"><strong>💬 访问博客</strong></a>&nbsp;&nbsp;|&nbsp;&nbsp;
    <a href="https://Music.MoeKoe.cn" target="blank"><strong>🏠 项目主页</strong></a>
  </p>
  <p align="center">
    <a href="https://github.com/iAJue/MoeKoeMusic/README.md" target="blank"><strong>🇨🇳 简体中文</strong></a>&nbsp;&nbsp;|&nbsp;&nbsp;
    <a href="https://github.com/iAJue/MoeKoeMusic/blob/main/docs/README_tw.md" target="blank"><strong>🇨🇳 繁体中文</strong></a>&nbsp;&nbsp;|&nbsp;&nbsp;
    <a href="https://github.com/iAJue/MoeKoeMusic/blob/main/docs/README_ja.md" target="blank"><strong>🇯🇵 日本語</strong></a>&nbsp;&nbsp;|&nbsp;&nbsp;
    <a href="https://github.com/iAJue/MoeKoeMusic/blob/main/docs/README_en.md" target="blank"><strong>🇺🇸 English</strong></a>&nbsp;&nbsp;|&nbsp;&nbsp;
    <a href="https://github.com/iAJue/MoeKoeMusic/blob/main/docs/README_ko.md" target="blank"><strong>🇰🇷 한국어</strong></a>&nbsp;&nbsp;|&nbsp;&nbsp;
    <a href="https://github.com/iAJue/MoeKoeMusic/blob/main/docs/README_ru.md" target="blank"><strong>🇷🇺 Русский</strong></a>
    <br />
    <br />
  </p>
</p>

![images](https://github.com/iAJue/MoeKoeMusic/raw/main/images/1.png)

## ❤️ 前言

早在10年前后的样子,那会在用网页版QQ的时候我就已经开始使用酷狗音乐了(也是十来年的老粉了),所以这些年收藏的歌曲全部都在上面.后来我也尝试开始使用网易云或QQ音乐,也尝试把酷狗的歌单导入进去,但是效果都不尽人意.我听的大多是日漫OP,好多歌曲都没办法找到.

兜兜转转最后还是回到酷狗,但是在Mac端的酷狗,时常可能会出现不能播放的情况,虽说界面没什么功能,但也挺好的.在网友的安利下,我现在一直是在酷狗的[概念版](https://t1.kugou.com/d2tBza3CSV2)上听歌,并且是市面上为数不多能免费听VIP歌曲的音乐播放软件了,力推.

我在我的个人介绍页面说我特别喜欢听歌,尤其是日漫OP.怎么证明呢?(之前我网页版歌单也年久失修了)那就自己开发一个音乐播放器.

## 🚀 架构

本项目已从 Electron 全量迁移至 **Tauri 2**：

- **前端**：Vue 3 + Vite（界面代码与 Electron 时代完全兼容）
- **后端**：Rust —— 窗口管理、系统托盘、全局快捷键、桌面歌词、深度链接、WebSocket 远程控制等原生能力全部由 Rust 实现
- **API 层**：酷狗接口的 Rust 移植版（`rust-api/`）以库形式**进程内直连**，前端通过 `invoke` 调用，不再启动本地 HTTP 服务、不占用端口

相比 Electron 版本：安装包体积从 ~85MB 降至 **10~15MB**，常驻内存约减半，且消除了 `webSecurity: false` 等安全隐患。

## ✨ 特性

- ✅ 使用 Vue.js 全家桶开发
- 🦀 Tauri 2 + Rust 后端，轻量、安全、快速
- 🔴 酷狗账号登录（扫码/手机/账号登录）
- 📃 支持歌词显示
- 📻 每日推荐歌曲
- 🚫🤝 无任何社交功能
- 🔗 官方服务器直连, 无任何第三方 API
- ✔️ 每日自动领取VIP, 登录就是VIP
- 🎨 主题色切换
- 👋 启动问候语
- ⚙️ 多平台支持
- 🛠 更多特性开发中

## 📢 Todo List

- [x] 📺 支持 MV 播放
- [x] 🌚 Light/Dark Mode 自动切换
- [x] 🖥️ 支持 PWA
- [x] ⌨️ 全局快捷键
- [x] 🤟 多语言支持
- [x] 📻 桌面歌词（透明窗/置顶/鼠标穿透/拖拽）
- [x] ⚙️ 系统架构优化（Electron → Tauri 全量迁移）
- [x] 🎶 歌曲、歌单/收藏、取消
- [x] 🔗 `moekoe://` 深度链接
- [x] 📡 WebSocket 远程控制（:6520，设置中开启）

更新日志请查看 [Commits](https://github.com/iAJue/MoeKoeMusic/commits/main/)

## 📦️ 安装

访问本项目的 [Releases](https://github.com/iAJue/MoeKoeMusic/releases) 页面下载适合你平台的安装包：

| 平台 | 格式 |
|---|---|
| Windows | NSIS 安装包（中英双语，支持按用户/按机器安装） |
| macOS | DMG（Intel / Apple Silicon） |
| Linux | AppImage / deb / rpm |

> Web 端部署：`rust-api/` 可编译为独立 HTTP 服务（`cargo build --release`，环境变量 `PORT` 指定端口），配合 `npm run build` 产出的静态文件即可自行托管，前端通过 `VITE_APP_API_URL` 指向 API 地址。

## ⚙️ 开发

### 环境要求

1. [Node.js](https://nodejs.org/en/) >= 20
2. [Rust](https://www.rust-lang.org/tools/install) stable 工具链（rustup 安装即可）
3. Tauri 2 平台依赖：
   - **Windows**：WebView2（Win10/11 自带）+ Visual Studio C++ 生成工具
   - **macOS**：Xcode Command Line Tools
   - **Linux**：`webkit2gtk-4.1`、`libappindicator3` 等，详见 [Tauri 官方文档](https://tauri.app/start/prerequisites/)

### 步骤

1. 克隆本仓库

```sh
git clone https://github.com/iAJue/MoeKoeMusic.git
```

2. 进入目录并安装依赖

```sh
cd MoeKoeMusic
npm install
```

3. 启动开发者模式（自动拉起 Vite 开发服务器并编译 Rust，首次编译需要几分钟）

```sh
npm run dev
```

4. 仅构建前端静态文件

```sh
npm run build
```

## 👷‍♂️ 编译客户端

如果在 Release 页面没有找到适合你的设备的安装包的话，你可以根据下面的步骤来打包自己的客户端。

1. 按上文「开发环境要求」装好 Node.js 与 Rust 工具链
2. 克隆仓库并 `npm install`
3. 执行打包：

```sh
npm run tauri:build
```

打包产物位于 `src-tauri/target/release/bundle/` 目录下，按当前系统自动生成对应格式（Windows 为 NSIS，macOS 为 DMG/App，Linux 为 AppImage/deb/rpm）。

- 指定打包格式：

```sh
npm run tauri:build -- --bundles nsis      # Windows NSIS
npm run tauri:build -- --bundles dmg      # macOS DMG
npm run tauri:build -- --bundles deb rpm  # Linux deb + rpm
```

- 指定目标架构（需先 `rustup target add` 对应 target）：

```sh
npm run tauri:build -- --target aarch64-apple-darwin   # Apple Silicon
npm run tauri:build -- --target x86_64-apple-darwin    # Intel Mac
npm run tauri:build -- --target aarch64-unknown-linux-gnu  # Linux ARM64
```

了解更多信息可访问 [Tauri 打包文档](https://tauri.app/distribute/)。

## ⭐ 支持项目

如果您觉得这个项目对您有帮助，欢迎给我们一个 Star！您的支持是我们持续改进的动力。

[![GitHub stars](https://img.shields.io/github/stars/iAJue/MoeKoeMusic.svg?style=social&label=Star)](https://github.com/iAJue/MoeKoeMusic)

## ✅ 反馈

如有任何问题或建议，欢迎提交 issue 或 pull request。

## ⚠️ 免责声明

0. 本程序是酷狗第三方客户端，并非酷狗官方，需要更完善的功能请下载官方客户端体验.
1. 本项目仅供学习使用，请尊重版权，请勿利用此项目从事商业行为及非法用途！
2. 使用本项目的过程中可能会产生版权数据。对于这些版权数据，本项目不拥有它们的所有权。为了避免侵权，使用者务必在 24 小时内清除使用本项目的过程中所产生的版权数据。
3. 由于使用本项目产生的包括由于本协议或由于使用或无法使用本项目而引起的任何性质的任何直接、间接、特殊、偶然或结果性损害（包括但不限于因商誉损失、停工、计算机故障或故障引起的损害赔偿，或任何及所有其他商业损害或损失）由使用者负责。
4. 禁止在违反当地法律法规的情况下使用本项目。对于使用者在明知或不知当地法律法规不允许的情况下使用本项目所造成的任何违法违规行为由使用者承担，本项目不承担由此造成的任何直接、间接、特殊、偶然或结果性责任。
5. 音乐平台不易，请尊重版权，支持正版。
6. 本项目仅用于对技术可行性的探索及研究，不接受任何商业（包括但不限于广告等）合作及捐赠。
7. 如果官方音乐平台觉得本项目不妥，可联系本项目更改或移除。

## 📜 开源许可

本项目仅供个人学习研究使用，禁止用于商业及非法用途。

基于 [GNU General Public License v2.0 (GPL-2.0)](https://github.com/iAJue/MoeKoeMusic/blob/main/LICENSE) 许可进行开源。

## 👍 灵感来源

API 源代码来自 [MakcRe/KuGouMusicApi](https://github.com/MakcRe/KuGouMusicApi)（本项目已将其移植为 Rust 版本，见 `rust-api/`）

- [Apple Music](https://music.apple.com)
- [YouTube Music](https://music.youtube.com)
- [YesPlayMusic](https://github.com/qier222/YesPlayMusic)
- [酷狗音乐](https://kugou.com/)

## 🖼️ 截图

![image](https://github.com/iAJue/MoeKoeMusic/raw/main/images/2.png)
![image](https://github.com/iAJue/MoeKoeMusic/raw/main/images/3.png)
![image](https://github.com/iAJue/MoeKoeMusic/raw/main/images/4.png)
![image](https://github.com/iAJue/MoeKoeMusic/raw/main/images/5.png)
![image](https://github.com/iAJue/MoeKoeMusic/raw/main/images/6.png)
![image](https://github.com/iAJue/MoeKoeMusic/raw/main/images/7.png)
![image](https://github.com/iAJue/MoeKoeMusic/raw/main/images/8.png)

## 🗓️ Star History

[![Star History Chart](https://api.star-history.com/svg?repos=iAJue/MoeKoeMusic&type=Date)](https://www.star-history.com/#iAJue/MoeKoeMusic&Date)
