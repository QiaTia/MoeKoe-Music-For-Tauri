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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DIR_SEQ: AtomicU64 = AtomicU64::new(0);

    /// 用临时目录的 store.json 隔离测试，避免污染真实用户数据。
    /// 每次调用生成独立目录（带自增序号），防止同进程内多个测试复用同一路径互相污染。
    fn temp_store() -> (SettingsStore, std::path::PathBuf) {
        let seq = TEST_DIR_SEQ.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "moekoe_test_store_{}_{}",
            std::process::id(),
            seq
        ));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("store.json");
        let store = SettingsStore::new();
        store.init(path.clone());
        (store, path)
    }

    #[test]
    fn test_get_default_null_when_empty() {
        let (store, _p) = temp_store();
        assert!(store.get("nonexistent").is_null());
        assert!(store.settings().is_null());
        assert_eq!(store.setting_str("language"), None);
        assert!(!store.setting_on("desktopLyrics"));
    }

    #[test]
    fn test_set_get_roundtrip_and_persist() {
        let (store, path) = temp_store();
        store.set("settings", json!({ "language": "en", "desktopLyrics": "on" }));
        // 落盘校验
        let raw = std::fs::read_to_string(&path).unwrap();
        let on_disk: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(on_disk["settings"]["language"], "en");
        assert!(store.setting_on("desktopLyrics"));
        assert_eq!(store.setting_str("language").as_deref(), Some("en"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_merge_setting_preserves_others() {
        let (store, path) = temp_store();
        store.set("settings", json!({ "language": "zh-CN", "theme": "pink" }));
        store.merge_setting("desktopLyrics", json!("on"));
        let s = store.settings();
        assert_eq!(s["language"], "zh-CN");
        assert_eq!(s["theme"], "pink");
        assert_eq!(s["desktopLyrics"], "on");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_init_loads_existing_file() {
        let dir = std::env::temp_dir().join(format!("moekoe_test_load_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("store.json");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"{\"settings\":{\"language\":\"ja\"}}").unwrap();
        drop(f);

        let store = SettingsStore::new();
        store.init(path.clone());
        assert_eq!(store.setting_str("language").as_deref(), Some("ja"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_clear_removes_file() {
        let (store, path) = temp_store();
        store.set("settings", json!({ "a": 1 }));
        assert!(path.exists());
        store.clear();
        assert!(store.get("settings").is_null());
        assert!(!path.exists());
    }

    #[test]
    fn test_t_language_fallback() {
        // 无法可靠控制 sys_locale，至少验证已知 key 命中
        let store = SettingsStore::new();
        store.init(std::env::temp_dir().join(format!("x_{}.json", std::process::id())));
        store.set("settings", json!({ "language": "en" }));
        assert_eq!(t(&store, "play"), "Play");
        assert_eq!(t(&store, "quit"), "Quit");
        // 未知 key 原样返回
        assert_eq!(t(&store, "no-such-key"), "no-such-key");
    }
}
