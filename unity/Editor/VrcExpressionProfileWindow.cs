using System;
using System.Collections.Generic;
using System.IO;
using UnityEditor;
using UnityEngine;

namespace VrcExpressionHotkeys.EditorTools
{
    /// <summary>
    /// Name an avatar's expressions, render face thumbnails, and export the
    /// profile folder the desktop app imports.
    /// </summary>
    public class VrcExpressionProfileWindow : EditorWindow
    {
        private GameObject _avatar;
        private string _avatarId = "";
        private string _avatarName = "Avatar";
        private readonly List<ProfileDocument.Row> _rows = new List<ProfileDocument.Row>();
        private Vector2 _scroll;

        [MenuItem("Tools/VRChat Expression Hotkeys/Avatar Profile")]
        public static void ShowWindow()
        {
            var window = GetWindow<VrcExpressionProfileWindow>("Expression Profile");
            window.minSize = new Vector2(560, 420);
            window.Show();
        }

        private void OnGUI()
        {
            EditorGUILayout.HelpBox(
                "1. Select the avatar (a VRChat avatar descriptor if the SDK is installed, otherwise any Animator).\n" +
                "2. Name each expression. Suggest fills parameters and clip names.\n" +
                "3. Generate previews. The avatar must be in the open scene.\n" +
                "4. Export a profile folder, then import it in VRChat Expression Hotkeys.",
                MessageType.Info);

            _avatar = (GameObject)EditorGUILayout.ObjectField("Avatar", _avatar, typeof(GameObject), true);
            if (GUILayout.Button("Use selected object"))
            {
                _avatar = Selection.activeGameObject;
                PullId();
            }
            _avatarName = EditorGUILayout.TextField("Display name", _avatarName);
            _avatarId = EditorGUILayout.TextField("Avatar / blueprint ID", _avatarId);

            GUILayout.BeginHorizontal();
            if (GUILayout.Button("Suggest from avatar"))
                Suggest();
            if (GUILayout.Button("Add expression"))
                _rows.Add(MakeRow(_rows.Count + 1, "Expression" + (_rows.Count + 1)));
            GUILayout.EndHorizontal();

            _scroll = GUILayout.BeginScrollView(_scroll);
            for (int i = 0; i < _rows.Count; i++)
                DrawRow(i);
            GUILayout.EndScrollView();

            if (GUILayout.Button("Generate previews (256)", GUILayout.Height(28)))
                GeneratePreviews();
            if (GUILayout.Button("Export profile folder", GUILayout.Height(28)))
                ExportPack();
        }

        private void DrawRow(int index)
        {
            var row = _rows[index];
            GUILayout.BeginVertical("box");
            GUILayout.BeginHorizontal();
            row.index = EditorGUILayout.IntField("Index", row.index);
            if (GUILayout.Button("Remove", GUILayout.Width(70)))
            {
                _rows.RemoveAt(index);
                GUIUtility.ExitGUI();
            }
            GUILayout.EndHorizontal();
            row.name = EditorGUILayout.TextField("Name", row.name);
            row.parameter = EditorGUILayout.TextField("Parameter", row.parameter);
            row.osc = EditorGUILayout.TextField("OSC address", row.osc);
            row.clip = (AnimationClip)EditorGUILayout.ObjectField(
                "Preview clip", row.clip, typeof(AnimationClip), false);
            GUILayout.EndVertical();
        }

        private void PullId()
        {
            if (_avatar == null)
                return;
            _avatarName = _avatar.name;
            var blueprint = VrcOptional.TryBlueprintId(_avatar);
            if (!string.IsNullOrEmpty(blueprint))
                _avatarId = blueprint;
            else if (string.IsNullOrEmpty(_avatarId))
                _avatarId = ProfileDocument.Sanitize(_avatar.name);
        }

        private void Suggest()
        {
            if (_avatar == null)
            {
                EditorUtility.DisplayDialog("Expression Profile", "Assign an avatar first.", "OK");
                return;
            }
            PullId();
            var seen = new HashSet<string>();
            var suggested = new List<string>();
            foreach (var name in VrcOptional.TryExpressionParameterNames(_avatar))
            {
                if (seen.Add(name))
                    suggested.Add(name);
            }
            var animator = _avatar.GetComponentInChildren<Animator>();
            if (animator != null)
            {
                foreach (var parameter in animator.parameters)
                {
                    if (parameter.type != AnimatorControllerParameterType.Bool
                        && parameter.type != AnimatorControllerParameterType.Int)
                        continue;
                    if (seen.Add(parameter.name))
                        suggested.Add(parameter.name);
                }
                var controller = animator.runtimeAnimatorController;
                if (controller != null)
                {
                    foreach (var clip in controller.animationClips)
                    {
                        if (clip != null && seen.Add(clip.name))
                            suggested.Add(clip.name);
                    }
                }
            }
            if (suggested.Count == 0)
            {
                EditorUtility.DisplayDialog(
                    "Expression Profile",
                    "No expression parameters or clips were found. Add rows by hand.",
                    "OK");
                return;
            }
            _rows.Clear();
            int index = 1;
            foreach (var name in suggested)
            {
                var row = MakeRow(index, name);
                if (animator != null && animator.runtimeAnimatorController != null)
                {
                    foreach (var clip in animator.runtimeAnimatorController.animationClips)
                    {
                        if (clip != null && clip.name == name)
                        {
                            row.clip = clip;
                            break;
                        }
                    }
                }
                _rows.Add(row);
                index++;
                if (index > 48)
                    break;
            }
        }

        private static ProfileDocument.Row MakeRow(int index, string name)
        {
            return new ProfileDocument.Row
            {
                index = index,
                name = name,
                parameter = name,
                osc = "/avatar/parameters/" + name
            };
        }

        private void GeneratePreviews()
        {
            if (_avatar == null)
            {
                EditorUtility.DisplayDialog(
                    "Expression Profile",
                    "Open the avatar in the scene and assign it here. Previews are rendered from that object.",
                    "OK");
                return;
            }
            string folder = EditorUtility.SaveFolderPanel("Preview folder", "", "thumbs");
            if (string.IsNullOrEmpty(folder))
                return;
            Directory.CreateDirectory(folder);
            int written = 0;
            foreach (var row in _rows)
            {
                if (row.clip == null)
                    continue;
                string path = Path.Combine(folder, row.index + ".png");
                if (ExpressionPreview.TryRender(_avatar, row.clip, path, 256))
                    written++;
            }
            EditorUtility.DisplayDialog(
                "Expression Profile",
                written + " preview PNG(s) written. Rows without a clip were skipped.",
                "OK");
        }

        private void ExportPack()
        {
            if (string.IsNullOrEmpty(_avatarId))
                PullId();
            if (string.IsNullOrEmpty(ProfileDocument.Sanitize(_avatarId)))
            {
                EditorUtility.DisplayDialog("Expression Profile", "Enter an avatar ID.", "OK");
                return;
            }
            string folder = EditorUtility.SaveFolderPanel(
                "Export profile folder", "", ProfileDocument.Sanitize(_avatarId));
            if (string.IsNullOrEmpty(folder))
                return;
            Directory.CreateDirectory(Path.Combine(folder, "thumbs"));
            foreach (var row in _rows)
            {
                row.thumb = "";
                if (row.clip == null || _avatar == null || row.index < 1)
                    continue;
                string png = Path.Combine(folder, "thumbs", row.index + ".png");
                if (ExpressionPreview.TryRender(_avatar, row.clip, png, 256))
                    row.thumb = "thumbs/" + row.index + ".png";
            }
            string json = ProfileDocument.Build(
                ProfileDocument.Sanitize(_avatarId), _avatarName, _avatarId, _rows);
            File.WriteAllText(Path.Combine(folder, "profile.json"), json);
            EditorUtility.DisplayDialog(
                "Expression Profile",
                "Wrote " + folder + "\nImport that folder (or a zip of it) in the desktop app.",
                "OK");
        }
    }

    public static class ProfileDocument
    {
        public class Row
        {
            public int index = 1;
            public string name = "Expression1";
            public string parameter = "Expression1";
            public string osc = "/avatar/parameters/Expression1";
            public string thumb = "";
            public AnimationClip clip;
        }

        public static string Build(string id, string displayName, string blueprintId, IList<Row> rows)
        {
            var expressions = new List<string>();
            if (rows != null)
            {
                foreach (var row in rows)
                {
                    if (row == null || row.index < 1 || string.IsNullOrEmpty(row.name))
                        continue;
                    expressions.Add(
                        "      {\"index\": " + row.index
                        + ", \"name\": " + Quote(row.name)
                        + ", \"parameter\": " + Quote(row.parameter)
                        + ", \"osc\": " + Quote(row.osc)
                        + ", \"type\": \"bool\", \"thumb\": " + Quote(row.thumb) + "}");
                }
            }
            return "{\n"
                + "  \"id\": " + Quote(id) + ",\n"
                + "  \"name\": " + Quote(displayName) + ",\n"
                + "  \"blueprint_id\": " + Quote(blueprintId) + ",\n"
                + "  \"expressions\": [\n" + string.Join(",\n", expressions) + "\n  ]\n}\n";
        }

        public static string Sanitize(string value)
        {
            if (string.IsNullOrEmpty(value))
                return "";
            var chars = new List<char>();
            foreach (var ch in value.Trim())
            {
                if (char.IsLetterOrDigit(ch) || ch == '.' || ch == '_' || ch == '-')
                    chars.Add(ch);
            }
            var text = new string(chars.ToArray());
            return text.Length > 128 ? text.Substring(0, 128) : text;
        }

        public static string Quote(string value)
        {
            if (value == null)
                value = "";
            return "\"" + value.Replace("\\", "\\\\").Replace("\"", "\\\"") + "\"";
        }
    }

    public static class ExpressionPreview
    {
        public static bool TryRender(GameObject avatar, AnimationClip clip, string pngPath, int size)
        {
            if (avatar == null || clip == null || string.IsNullOrEmpty(pngPath) || size < 1)
                return false;
            var clone = UnityEngine.Object.Instantiate(avatar);
            clone.hideFlags = HideFlags.HideAndDontSave;
            GameObject cameraObject = null;
            RenderTexture texture = null;
            Texture2D image = null;
            try
            {
                float at = clip.length > 0f ? clip.length * 0.5f : 0f;
                clip.SampleAnimation(clone, at);
                var animator = clone.GetComponentInChildren<Animator>();
                Transform head = null;
                if (animator != null && animator.isHuman)
                    head = animator.GetBoneTransform(HumanBodyBones.Head);
                if (head == null)
                    head = clone.transform;
                cameraObject = new GameObject("ExpressionPreviewCamera");
                cameraObject.hideFlags = HideFlags.HideAndDontSave;
                var camera = cameraObject.AddComponent<Camera>();
                camera.clearFlags = CameraClearFlags.SolidColor;
                camera.backgroundColor = new Color(0.15f, 0.15f, 0.15f, 1f);
                camera.nearClipPlane = 0.01f;
                camera.fieldOfView = 30f;
                camera.transform.position = head.position + head.forward * 0.35f + Vector3.up * 0.05f;
                camera.transform.LookAt(head.position);
                texture = new RenderTexture(size, size, 24);
                camera.targetTexture = texture;
                camera.Render();
                var previous = RenderTexture.active;
                RenderTexture.active = texture;
                image = new Texture2D(size, size, TextureFormat.RGB24, false);
                image.ReadPixels(new Rect(0, 0, size, size), 0, 0);
                image.Apply();
                RenderTexture.active = previous;
                Directory.CreateDirectory(Path.GetDirectoryName(pngPath) ?? ".");
                File.WriteAllBytes(pngPath, image.EncodeToPNG());
                return true;
            }
            catch (Exception error)
            {
                Debug.LogWarning("[ExpressionHotkeys] Preview failed: " + error.Message);
                return false;
            }
            finally
            {
                if (image != null)
                    UnityEngine.Object.DestroyImmediate(image);
                if (texture != null)
                {
                    texture.Release();
                    UnityEngine.Object.DestroyImmediate(texture);
                }
                if (cameraObject != null)
                    UnityEngine.Object.DestroyImmediate(cameraObject);
                UnityEngine.Object.DestroyImmediate(clone);
            }
        }
    }
}
