using System;
using System.Collections.Generic;
using System.Globalization;
using UnityEditor;
using UnityEngine;

namespace VrcExpressionHotkeys.EditorTools
{
    /// <summary>
    /// Editor language for the profile window. The choice is an EditorPref, so
    /// it survives a restart, and reading it does not reload the domain.
    /// Unity 2022 draws IMGUI with Inter, which has no Hangul, so this window
    /// switches its styles to a font that has Hangul and kana.
    /// </summary>
    public static class EditorLocale
    {
        static readonly string[] OsFonts =
        {
            "Malgun Gothic",
            "Apple SD Gothic Neo",
            "AppleGothic",
            "NanumGothic",
            "Noto Sans CJK KR",
            "Noto Sans CJK JP",
            "Noto Sans KR",
            "Source Han Sans KR",
            "Source Han Sans JP",
            "Yu Gothic UI",
            "Yu Gothic",
            "Meiryo",
            "Hiragino Sans",
            "WenQuanYi Micro Hei",
        };

        static string _current;
        static Font _font;
        static bool _fontReady;
        static int _depth;
        static readonly List<SavedStyle> Saved = new List<SavedStyle>();

        struct SavedStyle
        {
            public GUIStyle style;
            public Font font;
        }

        public static string Current
        {
            get
            {
                if (_current == null)
                    _current = Load();
                return _current;
            }
        }

        public static string T(string key)
        {
            return EditorStrings.Get(Current, key);
        }

        public static string F(string key, params object[] args)
        {
            return EditorStrings.Format(Current, key, args);
        }

        public static string Present(string stored)
        {
            return EditorStrings.Present(Current, stored);
        }

        public static void Set(string code)
        {
            code = EditorStrings.Normalize(code);
            _current = code;
            EditorPrefs.SetString(EditorStrings.PrefKey, code);
        }

        public static void BeginWindow()
        {
            _depth++;
            if (_depth != 1)
                return;
            var font = ReadyFont();
            if (font == null)
                return;
            Adopt(GUI.skin != null ? GUI.skin.label : null, font);
            Adopt(GUI.skin != null ? GUI.skin.button : null, font);
            Adopt(GUI.skin != null ? GUI.skin.textField : null, font);
            Adopt(GUI.skin != null ? GUI.skin.textArea : null, font);
            Adopt(GUI.skin != null ? GUI.skin.toggle : null, font);
            Adopt(GUI.skin != null ? GUI.skin.box : null, font);
            Adopt(GUI.skin != null ? GUI.skin.window : null, font);
            if (GUI.skin != null)
            {
                Saved.Add(new SavedStyle { style = null, font = GUI.skin.font });
                GUI.skin.font = font;
            }
            Adopt(EditorStyles.label, font);
            Adopt(EditorStyles.miniLabel, font);
            Adopt(EditorStyles.boldLabel, font);
            Adopt(EditorStyles.wordWrappedLabel, font);
            Adopt(EditorStyles.textField, font);
            Adopt(EditorStyles.textArea, font);
            Adopt(EditorStyles.popup, font);
            Adopt(EditorStyles.miniButton, font);
            Adopt(EditorStyles.helpBox, font);
            Adopt(EditorStyles.toolbarButton, font);
            Adopt(EditorStyles.objectField, font);
            Adopt(EditorStyles.foldout, font);
        }

        public static void EndWindow()
        {
            if (_depth <= 0)
                return;
            _depth--;
            if (_depth != 0)
                return;
            for (int i = Saved.Count - 1; i >= 0; i--)
            {
                var saved = Saved[i];
                if (saved.style == null)
                {
                    if (GUI.skin != null)
                        GUI.skin.font = saved.font;
                }
                else
                {
                    saved.style.font = saved.font;
                }
            }
            Saved.Clear();
        }

        static void Adopt(GUIStyle style, Font font)
        {
            if (style == null)
                return;
            Saved.Add(new SavedStyle { style = style, font = style.font });
            style.font = font;
        }

        static string Load()
        {
            if (EditorPrefs.HasKey(EditorStrings.PrefKey))
                return EditorStrings.Normalize(EditorPrefs.GetString(EditorStrings.PrefKey, "ja"));
            string editor = EditorPrefs.GetString("EditorLanguage", "");
            if (string.IsNullOrEmpty(editor))
                editor = EditorPrefs.GetString("editor_locale", "");
            string system = Application.systemLanguage.ToString();
            var culture = CultureInfo.CurrentUICulture;
            string chosen = EditorStrings.Infer(editor, system, culture.TwoLetterISOLanguageName, culture.Name);
            EditorPrefs.SetString(EditorStrings.PrefKey, chosen);
            return chosen;
        }

        static Font ReadyFont()
        {
            if (_fontReady)
                return _font;
            _fontReady = true;
            _font = PickFont();
            return _font;
        }

        static Font PickFont()
        {
            Font hangul = null;
            Font kana = null;
            for (int i = 0; i < OsFonts.Length; i++)
            {
                var font = TryOs(OsFonts[i]);
                if (font == null)
                    continue;
                bool hasHangul = Has(font, "한");
                bool hasKana = Has(font, "あ");
                if (hasHangul && hasKana)
                    return font;
                if (hasHangul && hangul == null)
                    hangul = font;
                if (hasKana && kana == null)
                    kana = font;
            }
            var bundled = LoadBundled();
            if (bundled != null && Has(bundled, "한"))
                return bundled;
            if (hangul != null)
                return hangul;
            if (kana != null)
                return kana;
            return bundled;
        }

        static bool Has(Font font, string sample)
        {
            try
            {
                font.RequestCharactersInTexture(sample, 16);
                return font.HasCharacter(sample[0]);
            }
            catch (Exception)
            {
                return false;
            }
        }

        static Font TryOs(string name)
        {
            try
            {
                return Font.CreateDynamicFontFromOSFont(name, 16);
            }
            catch (Exception)
            {
                return null;
            }
        }

        static Font LoadBundled()
        {
            var guids = AssetDatabase.FindAssets("EditorHangul");
            for (int i = 0; i < guids.Length; i++)
            {
                string path = AssetDatabase.GUIDToAssetPath(guids[i]);
                if (string.IsNullOrEmpty(path) || !path.EndsWith(".ttf", StringComparison.OrdinalIgnoreCase))
                    continue;
                var font = AssetDatabase.LoadAssetAtPath<Font>(path);
                if (font != null)
                    return font;
            }
            return null;
        }
    }
}
