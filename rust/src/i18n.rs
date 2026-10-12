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
        "subtitle" => {
            if ja {
                "VRChat の表情ホットキー (F1-F4)"
            } else {
                "VRChat expression hotkeys (F1-F4)"
            }
        }
        "host" => {
            if ja {
                "ホスト"
            } else {
                "Host"
            }
        }
        "port" => {
            if ja {
                "ポート"
            } else {
                "Port"
            }
        }
        "language" => {
            if ja {
                "言語"
            } else {
                "Language"
            }
        }
        "zoom" => {
            if ja {
                "表示サイズ"
            } else {
                "Text size"
            }
        }
        "port_invalid" => {
            if ja {
                "1〜65535 の数字を入れてください。"
            } else {
                "Enter a number from 1 to 65535."
            }
        }
        "host_invalid" => {
            if ja {
                "ホスト名か IP アドレスを入れてください。"
            } else {
                "Enter a host name or an IP address."
            }
        }
        "key_permission" => {
            if ja {
                "キーボードを読めません。ユーザーを input グループに入れて、ログインし直してください。端末での例: sudo usermod -aG input \"$USER\""
            } else {
                "The keyboard cannot be read. Add your user to the input group, then sign in again. Example: sudo usermod -aG input \"$USER\""
            }
        }
        "key_none" => {
            if ja {
                "キーボードが見つかりません。接続を確認し、待受を一度止めてから入れ直してください。"
            } else {
                "No keyboard was found. Check the connection, then turn listening off and on."
            }
        }
        "key_unsupported" => {
            if ja {
                "この OS ではキーの待受に対応していません。割り当ては編集できます。"
            } else {
                "This OS cannot listen for keys. Assignments can still be edited."
            }
        }
        "none" => {
            if ja {
                "(なし)"
            } else {
                "(none)"
            }
        }
        "import_bad_pack" => {
            if ja {
                "読み込めるのは zip、フォルダ、profile.json です。"
            } else {
                "Import a .zip, a folder, or profile.json."
            }
        }
        "import_unsafe" => {
            if ja {
                "プロファイルの中に安全でないパスがあります。"
            } else {
                "The profile pack contains an unsafe path."
            }
        }
        "import_missing" => {
            if ja {
                "profile.json が見つかりません。"
            } else {
                "profile.json was not found."
            }
        }
        _ => key_fallback(key),
    }
}

fn key_fallback(key: &str) -> &'static str {
    match key {
        "enable" => "Listen for Shift+F1..F4",
        _ => "?",
    }
}

pub fn present_error(lang: &str, err: &str) -> String {
    let key = match err {
        "profile pack must be a .zip, a folder, or profile.json" => "import_bad_pack",
        "unsafe path in profile pack" => "import_unsafe",
        "profile.json not found" => "import_missing",
        _ => return err.to_string(),
    };
    tr(lang, key).to_string()
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
