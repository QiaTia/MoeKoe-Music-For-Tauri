//! 应用设置存储 —— 对齐 electron-store 的语义。
//!
//! 存储文件：`app_config_dir/store.json`
//! 顶层键与 Electron 版一致：settings / windowState / maximize /
//! lyricsWindowPosition / lyricsWindowSize / disclaimerAccepted

use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::RwLock;

pub struct SettingsStore {
    path: RwLock<Option<PathBuf>>,
    data: RwLock<Value>,
}

impl SettingsStore {
    pub fn new() -> Self {
        SettingsStore {
            path: RwLock::new(None),
            data: RwLock::new(json!({})),
        }
    }

    /// 绑定磁盘路径并加载
    pub fn init(&self, path: PathBuf) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let loaded = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .unwrap_or(json!({}));
        if let Ok(mut g) = self.data.write() {
            *g = loaded;
        }
        if let Ok(mut g) = self.path.write() {
            *g = Some(path);
        }
    }

    fn persist(&self) {
        let path = match self.path.read().ok().and_then(|g| g.clone()) {
            Some(p) => p,
            None => return,
        };
        if let Ok(g) = self.data.read() {
            if let Ok(text) = serde_json::to_string_pretty(&*g) {
                let _ = std::fs::write(path, text);
            }
        }
    }

    /// 读取顶层键
    pub fn get(&self, key: &str) -> Value {
        self.data
            .read()
            .ok()
            .map(|g| g.get(key).cloned().unwrap_or(Value::Null))
            .unwrap_or(Value::Null)
    }

    /// 写入顶层键并落盘
    pub fn set(&self, key: &str, value: Value) {
        if let Ok(mut g) = self.data.write() {
            if let Some(obj) = g.as_object_mut() {
                obj.insert(key.to_string(), value);
            } else {
                *g = json!({ key: value });
            }
        }
        self.persist();
    }

    /// 清空全部并删除文件（clear-settings）
    pub fn clear(&self) {
        if let Ok(mut g) = self.data.write() {
            *g = json!({});
        }
        if let Some(p) = self.path.read().ok().and_then(|g| g.clone()) {
            let _ = std::fs::remove_file(p);
        }
    }

    /// settings 对象
    pub fn settings(&self) -> Value {
        self.get("settings")
    }

    /// settings 内某项字符串值
    pub fn setting_str(&self, name: &str) -> Option<String> {
        self.settings()
            .get(name)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    /// settings 内某项是否为 "on"
    pub fn setting_on(&self, name: &str) -> bool {
        self.setting_str(name).as_deref() == Some("on")
    }

    /// 合并写 settings 中的单项（如 desktopLyrics 开关同步）
    pub fn merge_setting(&self, name: &str, value: Value) {
        let mut settings = self.settings();
        if !settings.is_object() {
            settings = json!({});
        }
        if let Some(obj) = settings.as_object_mut() {
            obj.insert(name.to_string(), value);
        }
        self.set("settings", settings);
    }
}

/// 迷你 i18n（托盘/通知文案），对齐 electron/language/i18n.js
pub fn t(store: &SettingsStore, key: &str) -> String {
    let lang = store
        .setting_str("language")
        .unwrap_or_else(|| {
            let sys = sys_locale::get_locale().unwrap_or_default();
            if sys.starts_with("zh") {
                if sys.contains("TW") || sys.contains("HK") {
                    "zh-TW".into()
                } else {
                    "zh-CN".into()
                }
            } else if sys.starts_with("ru") {
                "ru".into()
            } else if sys.starts_with("ja") {
                "ja".into()
            } else if sys.starts_with("ko") {
                "ko".into()
            } else if sys.starts_with("en") {
                "en".into()
            } else {
                "zh-CN".into()
            }
        });

    let table: &[(&str, &str)] = match lang.as_str() {
        "zh-TW" => &[
            ("project-home", "專案首頁"),
            ("report-bug", "回報問題"),
            ("prev-track", "上一首"),
            ("pause", "暫停"),
            ("play", "播放"),
            ("next-track", "下一首"),
            ("check-updates", "檢查更新"),
            ("restart-app", "重新啟動"),
            ("show-hide", "顯示/隱藏"),
            ("quit", "結束程式"),
            ("desktop-lyrics-closed", "桌面歌詞已關閉"),
            ("now-playing", "正在播放："),
        ],
        "en" => &[
            ("project-home", "Project Homepage"),
            ("report-bug", "Report Bug"),
            ("prev-track", "Previous"),
            ("pause", "Pause"),
            ("play", "Play"),
            ("next-track", "Next"),
            ("check-updates", "Check Updates"),
            ("restart-app", "Restart App"),
            ("show-hide", "Show/Hide"),
            ("quit", "Quit"),
            ("desktop-lyrics-closed", "Desktop Lyrics Closed"),
            ("now-playing", "Now Playing: "),
        ],
        "ru" => &[
            ("project-home", "Страница проекта"),
            ("report-bug", "Сообщить об ошибке"),
            ("prev-track", "Предыдущий"),
            ("pause", "Пауза"),
            ("play", "Воспроизвести"),
            ("next-track", "Следующий"),
            ("check-updates", "Проверить обновления"),
            ("restart-app", "Перезапустить"),
            ("show-hide", "Показать/Скрыть"),
            ("quit", "Выход"),
            ("desktop-lyrics-closed", "Текст на рабочем столе скрыт"),
            ("now-playing", "Сейчас играет: "),
        ],
        "ja" => &[
            ("project-home", "プロジェクトホーム"),
            ("report-bug", "バグ報告"),
            ("prev-track", "前の曲"),
            ("pause", "一時停止"),
            ("play", "再生"),
            ("next-track", "次の曲"),
            ("check-updates", "更新を確認"),
            ("restart-app", "再起動"),
            ("show-hide", "表示/非表示"),
            ("quit", "終了"),
            ("desktop-lyrics-closed", "デスクトップ歌詞を閉じました"),
            ("now-playing", "再生中："),
        ],
        "ko" => &[
            ("project-home", "프로젝트 홈"),
            ("report-bug", "버그 신고"),
            ("prev-track", "이전"),
            ("pause", "일시정지"),
            ("play", "재생"),
            ("next-track", "다음"),
            ("check-updates", "업데이트 확인"),
            ("restart-app", "재시작"),
            ("show-hide", "표시/숨기기"),
            ("quit", "종료"),
            ("desktop-lyrics-closed", "바탕화면 가사 닫힘"),
            ("now-playing", "재생 중: "),
        ],
        _ => &[
            ("project-home", "项目主页"),
            ("report-bug", "反馈bug"),
            ("prev-track", "上一首"),
            ("pause", "暂停"),
            ("play", "播放"),
            ("next-track", "下一首"),
            ("check-updates", "检查更新"),
            ("restart-app", "重启应用"),
            ("show-hide", "显示/隐藏"),
            ("quit", "退出程序"),
            ("desktop-lyrics-closed", "桌面歌词已关闭"),
            ("now-playing", "正在播放："),
        ],
    };

    table
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v.to_string())
        .unwrap_or_else(|| key.to_string())
}
