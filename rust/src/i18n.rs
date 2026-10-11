pub fn tr(lang: &str, key: &str) -> &'static str {
    let ja = lang == "ja";
    match key {
        "enable" => {
            if ja {
                "Shift+F1..F4 を待つ"
            } else {
                "Listen for Shift+F1..F4"
            }
        }
        "filter" => {
            if ja {
                "VRChat が前面のときだけ"
            } else {
                "Only while VRChat is in front"
            }
        }
        "avatar" => {
            if ja {
                "アバター"
            } else {
                "Avatar"
            }
        }
        "import" => {
            if ja {
                "アバタープロファイルを読み込む"
            } else {
                "Import avatar profile"
            }
        }
        "listen" => {
            if ja {
                "アバター OSC ポート"
            } else {
                "Avatar OSC port"
            }
        }
        "unassigned" => {
            if ja {
                "(未割り当て)"
            } else {
                "(unassigned)"
            }
        }
        "busy" => {
            if ja {
                "アバター OSC ポートが使用中です。OSC ルーターで転送してください。"
            } else {
                "Avatar OSC port is busy. Forward /avatar/change with an OSC router."
            }
        }
        "ok" => {
            if ja {
                "/avatar/change を待受中"
            } else {
                "Listening for /avatar/change"
            }
        }
        "running" => {
            if ja {
                "待受中"
            } else {
                "Listening"
            }
        }
        "stopped" => {
            if ja {
                "停止"
            } else {
                "Stopped"
            }
        }
        "last" => {
            if ja {
                "最後"
            } else {
                "Last"
            }
        }
        "tray_open" => {
            if ja {
                "画面を開く"
            } else {
                "Open window"
            }
        }
        "tray_start" => {
            if ja {
                "待受の開始"
            } else {
                "Start listening"
            }
        }
        "tray_stop" => {
            if ja {
                "待受の停止"
            } else {
                "Stop listening"
            }
        }
        "tray_quit" => {
            if ja {
                "終了"
            } else {
                "Quit"
            }
        }
        "preview" => {
            if ja {
                "全画面で見る"
            } else {
                "Preview fullscreen"
            }
        }
        "stage_close" => {
            if ja {
                "閉じる"
            } else {
                "Close"
            }
        }
        "subtitle" => "VRChat Expression Hotkeys (F1-F4)",
        _ => key_fallback(key),
    }
}

fn key_fallback(key: &str) -> &'static str {
    match key {
        "enable" => "Listen for Shift+F1..F4",
        _ => "?",
    }
}

pub fn import_notice(skipped: &[i64], lang: &str) -> String {
    if skipped.is_empty() {
        return String::new();
    }
    let indexes = skipped
        .iter()
        .map(|index| index.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    if lang == "ja" {
        format!("無料版は表情 4 つまでです。{indexes} は読み飛ばしました。")
    } else {
        format!("Free edition keeps 4 expressions. Ignored indexes: {indexes}.")
    }
}
