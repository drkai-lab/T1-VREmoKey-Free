using System.Collections.Generic;
using System.Globalization;

namespace VrcExpressionHotkeys.EditorTools
{
    /// <summary>
    /// The only user-visible string table for the editor window. Each key has
    /// Japanese, English, and Korean. Expression names the user typed are not
    /// in this table.
    /// </summary>
    public static class EditorStrings
    {
        public const string PrefKey = "VREmoKey.EditorLang";

        static readonly string[][] LanguageHints =
        {
            new[] { "ja", "ja", "japanese", "jp" },
            new[] { "en", "en", "english" },
            new[] { "ko", "ko", "korean", "kr" },
        };

        static readonly string[][] Rows =
        {
            new[] { "window.title", "表情プロファイル", "Expression Profile", "표정 프로필" },
            new[] { "label.language", "言語", "Language", "언어" },
            new[] { "lang.ja", "日本語", "日本語", "日本語" },
            new[] { "lang.en", "English", "English", "English" },
            new[] { "lang.ko", "한국어", "한국어", "한국어" },
            new[] { "tooltip.lang", "表示言語です。切り替えてもエディタは再起動しません。", "Display language. Switching does not restart the editor.", "표시 언어입니다. 바꿔도 에디터를 다시 시작하지 않습니다." },
            new[] { "help.body", "無料版は表情 4 つ (Shift+F1 から F4) です。\n表情を探す: FX とアバターのフォルダから、顔の blendShape を動かすクリップを候補にします。\nまばたき、Viseme、口リセット、手のポーズだけ、空のクリップ、DefaultFace 相当は理由付きで並びます。戻すで候補に戻せます。\n名前の欄は翻訳しません。選んだスロットに bool と FX レイヤーを足し、手のジェスチャーより後ろに置きます。Write Defaults は既存に合わせ、同期ビットを確認します。\n最初のバックアップは Assets の外に残ります。元に戻すで戻せます。同じレイヤーは二重に追加しません。書き出す前と元に戻す前に確認します。", "Free edition: 4 expressions (Shift+F1 through F4).\nFind expressions: clips from the FX controller and the avatar folder that move a face blend shape.\nBlinks, visemes, mouth resets, hand-only poses, empty clips, and DefaultFace equivalents are listed with a reason. Restore puts one back.\nThe name field is not translated. Install adds a bool and an FX layer for each chosen slot, after hand gestures. Write Defaults follows the existing controller, after a synced-bit check.\nThe first backup stays outside Assets. Restore backup copies it back. The same layer is not added twice. Install and restore ask before they overwrite the controller.", "무료 버전은 표정이 4개(Shift+F1부터 F4)입니다.\n표정 찾기: FX와 아바타 폴더에서 얼굴 블렌드셰이프를 움직이는 클립을 후보로 둡니다.\n눈 깜빡임, 비셈, 입 모양 초기화, 손 포즈만, 빈 클립, DefaultFace에 해당하는 클립은 이유와 함께 나옵니다. 다시 넣기로 후보로 되돌립니다.\n이름 칸은 번역하지 않습니다. 고른 슬롯에 bool과 FX 레이어를 넣고, 손 제스처보다 뒤에 둡니다. Write Defaults는 기존과 맞추고, 동기화 비트를 확인합니다.\n첫 백업은 Assets 밖에 남습니다. 되돌리기로 복구합니다. 같은 레이어는 두 번 넣지 않습니다. 내보내기와 되돌리기 전에 확인합니다." },
            new[] { "field.avatar", "アバター", "Avatar", "아바타" },
            new[] { "tooltip.avatar", "シーンに置いたアバターです。", "The avatar placed in the scene.", "씬에 놓은 아바타입니다." },
            new[] { "button.use_selected", "選択中のオブジェクトを使う", "Use selected object", "선택한 오브젝트 사용" },
            new[] { "tooltip.use_selected", "ヒエラルキーで選んでいるオブジェクトをアバターにします。", "Use the Hierarchy selection as the avatar.", "하이어라키에서 선택한 오브젝트를 아바타로 씁니다." },
            new[] { "field.display_name", "表示名", "Display name", "표시 이름" },
            new[] { "tooltip.display_name", "プロファイルに書くアバター名です。翻訳しません。", "Avatar name stored in the profile. Not translated.", "프로필에 저장할 아바타 이름입니다. 번역하지 않습니다." },
            new[] { "field.avatar_id", "アバター / ブループリント ID", "Avatar / blueprint ID", "아바타 / 블루프린트 ID" },
            new[] { "tooltip.avatar_id", "アプリがアバターを見分ける ID です。", "Id the app uses to tell avatars apart.", "앱이 아바타를 구분하는 ID입니다." },
            new[] { "button.find", "表情を探す", "Find expressions", "표정 찾기" },
            new[] { "tooltip.find", "FX とアバターのフォルダから顔のクリップを探します。", "Search the FX controller and the avatar folder for face clips.", "FX와 아바타 폴더에서 얼굴 클립을 찾습니다." },
            new[] { "button.install", "FX に入れて書き出す", "Install and export", "FX에 넣고 내보내기" },
            new[] { "tooltip.install", "選んだスロットの bool とレイヤーを足し、プロファイルを書き出します。", "Add bools and layers for the chosen slots, then write the profile.", "고른 슬롯의 bool과 레이어를 넣고 프로필을 저장합니다." },
            new[] { "button.restore_backup", "元に戻す", "Restore backup", "되돌리기" },
            new[] { "tooltip.restore_backup", "最初の編集の前に取ったバックアップを戻します。", "Copy back the backup taken before the first edit.", "처음 수정하기 전에 만든 백업을 되돌립니다." },
            new[] { "field.name", "名前", "Name", "이름" },
            new[] { "tooltip.name", "アプリに出る表情名です。この欄は翻訳しません。", "Expression name shown in the app. This field is not translated.", "앱에 보이는 표정 이름입니다. 이 칸은 번역하지 않습니다." },
            new[] { "field.slot", "スロット", "Slot", "슬롯" },
            new[] { "slot.none", "(なし)", "(none)", "(없음)" },
            new[] { "button.exclude", "除外", "Exclude", "제외" },
            new[] { "tooltip.exclude", "このクリップを書き出しから外します。", "Leave this clip out of the export.", "이 클립을 내보내기에서 뺍니다." },
            new[] { "field.excluded", "除外理由", "Excluded", "제외 이유" },
            new[] { "button.restore_clip", "戻す", "Restore", "다시 넣기" },
            new[] { "tooltip.restore_clip", "除外を取り消して候補に戻します。", "Put this clip back into the candidates.", "제외를 취소하고 후보로 되돌립니다." },
            new[] { "preview.none", "(プレビューなし)", "(no preview)", "(미리보기 없음)" },
            new[] { "button.ok", "OK", "OK", "확인" },
            new[] { "button.cancel", "キャンセル", "Cancel", "취소" },
            new[] { "dialog.need_avatar", "先にアバターを指定してください。", "Assign an avatar first.", "먼저 아바타를 지정하세요." },
            new[] { "dialog.need_fx", "FX コントローラーがあるアバターで、先に表情を探してください。", "Find expressions on an avatar with an FX controller first.", "FX 컨트롤러가 있는 아바타에서 먼저 표정을 찾으세요." },
            new[] { "dialog.need_id", "アバター ID を入力してください。", "Enter an avatar ID.", "아바타 ID를 입력하세요." },
            new[] { "dialog.need_slot", "Shift+F1 から F4 のどれかに、表情を 1 つ以上割り当ててください。", "Assign at least one expression to Shift+F1 through F4.", "Shift+F1부터 F4 사이에 표정을 하나 이상 할당하세요." },
            new[] { "dialog.save_title", "プロファイルのフォルダ", "Export profile folder", "프로필 폴더" },
            new[] { "dialog.installed", "レイヤーを {0} 個追加し、{1} に書き出しました。\nデスクトップアプリでこのフォルダ (または zip) を読み込んでください。", "Installed {0} layer(s) and wrote {1}.\nImport that folder (or a zip of it) in the desktop app.", "레이어 {0}개를 넣고 {1}에 저장했습니다.\n데스크톱 앱에서 이 폴더(또는 zip)를 불러오세요." },
            new[] { "dialog.restored", "VREmoKeyBackup から {0} 個のアセットを戻しました。", "Restored {0} asset(s) from VREmoKeyBackup.", "VREmoKeyBackup에서 에셋 {0}개를 되돌렸습니다." },
            new[] { "dialog.no_backup", "バックアップが見つかりません。", "No backup was found.", "백업을 찾지 못했습니다." },
            new[] { "dialog.confirm_install", "選んだスロットの bool と FX レイヤーを追加し、プロファイルを書き出します。同名のレイヤーがあるときは上書きします。追加しますか?", "This adds bools and FX layers for the chosen slots, then writes the profile. A layer that already uses one of those names is overwritten. Continue?", "고른 슬롯의 bool과 FX 레이어를 넣고 프로필을 저장합니다. 같은 이름의 레이어가 있으면 내용을 덮어씁니다. 계속할까요?" },
            new[] { "dialog.confirm_restore", "最初の編集の前のバックアップで、FX コントローラーとパラメータを上書きします。その後の編集は消えます。上書きの前に、現在のファイルを VREmoKeyBackup の before-restore フォルダへコピーします。", "This replaces the FX controller and expression parameters with the backup from the first edit. Later changes are lost. Current files are copied to a before-restore folder under VREmoKeyBackup first.", "처음 수정하기 전의 백업으로 FX 컨트롤러와 파라미터를 덮어씁니다. 그 뒤의 수정은 사라집니다. 덮어쓰기 전에 현재 파일을 VREmoKeyBackup의 before-restore 폴더로 복사합니다." },
            new[] { "reason.empty", "空のクリップ", "Empty clip", "빈 클립" },
            new[] { "reason.hand", "手のポーズだけ", "Hand pose only", "손 포즈만" },
            new[] { "reason.mouth_reset", "口リセット", "Mouth reset", "입 모양 초기화" },
            new[] { "reason.blink", "まばたき", "Blink", "눈 깜빡임" },
            new[] { "reason.viseme", "Viseme", "Viseme", "비셈" },
            new[] { "reason.default_face", "DefaultFace 相当", "DefaultFace equivalent", "DefaultFace에 해당" },
            new[] { "reason.not_face", "顔の blendShape を動かさない", "Does not move a face blend shape", "얼굴 블렌드셰이프를 움직이지 않음" },
            new[] { "reason.expression", "表情", "Expression", "표정" },
            new[] { "status.summary", "候補 {0}、除外 {1}。Write Defaults は{2}。同期ビット {3} (bool 4 つで +4、上限 {4})。", "{0} expressions, {1} excluded. Write Defaults {2}. Synced bits {3} (4 bools need 4 more, limit {4}).", "후보 {0}개, 제외 {1}개. Write Defaults는 {2}. 동기화 비트 {3} (bool 4개면 4 추가, 상한 {4})." },
            new[] { "wd.on", "オン", "on", "켜짐" },
            new[] { "wd.off", "オフ", "off", "꺼짐" },
            new[] { "bits.unknown", "不明", "unknown", "알 수 없음" },
            new[] { "status.sdk", "VRChat SDK を検出しました。", "VRChat SDK detected.", "VRChat SDK를 찾았습니다." },
            new[] { "err.slot_limit", "無料版は 4 スロットまでです。", "The free edition keeps 4 slots.", "무료 버전은 슬롯이 4개까지입니다." },
            new[] { "err.not_bool", "{0} は bool ではありません。", "{0} is not a bool.", "{0}은(는) bool이 아닙니다." },
            new[] { "err.foreign", "{0} というレイヤーが既にあるため追加しません。", "A layer named {0} already exists, so nothing was added.", "{0} 레이어가 이미 있어서 추가하지 않습니다." },
            new[] { "err.sync", "同期ビットが {0} を超えます (現在 {1}、追加 {2})。", "Synced bits would pass {0} (now {1}, adding {2}).", "동기화 비트가 {0}을 넘습니다 (현재 {1}, 추가 {2})." },
            new[] { "err.no_fx", "FX コントローラーがありません。", "There is no FX controller.", "FX 컨트롤러가 없습니다." },
            new[] { "err.need_avatar", "アバターを選んでください。", "Choose an avatar.", "아바타를 선택하세요." },
            new[] { "err.no_fx_scan", "FX コントローラーが見つかりません。フォルダ内のクリップだけ探します。", "No FX controller was found. Only clips in the folder are searched.", "FX 컨트롤러가 없습니다. 폴더 안의 클립만 찾습니다." },
            new[] { "log.preview_failed", "プレビューに失敗しました: {0}", "Preview failed: {0}", "미리보기에 실패했습니다: {0}" },
        };

        static readonly Dictionary<string, string[]> Table = Build();

        static Dictionary<string, string[]> Build()
        {
            var table = new Dictionary<string, string[]>();
            for (int i = 0; i < Rows.Length; i++)
                table[Rows[i][0]] = Rows[i];
            return table;
        }

        public static string Get(string lang, string key)
        {
            if (key == null || !Table.TryGetValue(key, out var row))
                return key ?? "";
            int index = lang == "en" ? 2 : lang == "ko" ? 3 : 1;
            return row[index];
        }

        public static string Format(string lang, string key, params object[] args)
        {
            return string.Format(CultureInfo.InvariantCulture, Get(lang, key), args);
        }

        public static string Present(string lang, string stored)
        {
            if (string.IsNullOrEmpty(stored) || !stored.StartsWith("err:"))
                return stored ?? "";
            var parts = stored.Split(':');
            if (parts.Length < 2)
                return stored;
            string key = "err." + parts[1];
            if (parts.Length == 2)
                return Get(lang, key);
            var args = new object[parts.Length - 2];
            for (int i = 2; i < parts.Length; i++)
                args[i - 2] = parts[i];
            return Format(lang, key, args);
        }

        public static string Normalize(string raw)
        {
            var code = Map(raw);
            return code ?? "ja";
        }

        public static string Infer(params string[] hints)
        {
            if (hints != null)
            {
                for (int i = 0; i < hints.Length; i++)
                {
                    var code = Map(hints[i]);
                    if (code != null)
                        return code;
                }
            }
            return "ja";
        }

        static string Map(string raw)
        {
            if (string.IsNullOrEmpty(raw))
                return null;
            string folded = raw.Trim().ToLowerInvariant().Replace('_', '-');
            int dash = folded.IndexOf('-');
            string primary = dash > 0 ? folded.Substring(0, dash) : folded;
            for (int i = 0; i < LanguageHints.Length; i++)
            {
                var row = LanguageHints[i];
                for (int j = 1; j < row.Length; j++)
                {
                    if (primary == row[j] || folded == row[j])
                        return row[0];
                }
            }
            return null;
        }
    }
}
