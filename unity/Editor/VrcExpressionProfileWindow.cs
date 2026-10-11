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
        class Candidate
        {
            public AnimationClip clip;
            public string displayName = "";
            public string kind = "";
            public string reason = "";
            public bool suggested;
            public bool include;
            public int slot;
            public Texture2D thumb;
            public string thumbPath = "";
        }

        GameObject _avatar;
        string _avatarId = "";
        string _avatarName = "Avatar";
        readonly List<Candidate> _found = new List<Candidate>();
        ExpressionScan.Result _scan;
        string _status = "";
        Vector2 _scroll;

        [MenuItem("Tools/VRChat Expression Hotkeys/Avatar Profile")]
        public static void ShowWindow()
        {
            var window = GetWindow<VrcExpressionProfileWindow>("Expression Profile");
            window.minSize = new Vector2(640, 520);
            window.Show();
        }

        void OnDisable()
        {
            ClearThumbs();
        }

        void OnGUI()
        {
            EditorGUILayout.HelpBox(
                "Free edition: 4 expressions (Shift+F1 through F4).\n" +
                "表情を探す: face clips from the FX controller and the avatar folder.\n" +
                "Blinks, visemes, mouth resets, hand-only poses, empty clips, and DefaultFace are listed with a reason. Restore puts one back.\n" +
                "Display names can be Japanese. Install adds Expression1..4 bools and FX layers above hand gestures, matching Write Defaults, after a sync-bit check.\n" +
                "A backup is kept outside Assets. 元に戻す copies it back. The same install does not add a second copy.",
                MessageType.Info);

            _avatar = (GameObject)EditorGUILayout.ObjectField("Avatar", _avatar, typeof(GameObject), true);
            if (GUILayout.Button("Use selected object"))
            {
                _avatar = Selection.activeGameObject;
                PullId();
            }
            _avatarName = EditorGUILayout.TextField("Display name", _avatarName);
            _avatarId = EditorGUILayout.TextField("Avatar / blueprint ID", _avatarId);
            if (!string.IsNullOrEmpty(_status))
                EditorGUILayout.HelpBox(_status, MessageType.None);

            if (GUILayout.Button("Find expressions / 表情を探す", GUILayout.Height(28)))
                FindExpressions();

            _scroll = GUILayout.BeginScrollView(_scroll);
            for (int i = 0; i < _found.Count; i++)
                DrawCandidate(i);
            GUILayout.EndScrollView();

            if (GUILayout.Button("Install and export / FX に入れて書き出す", GUILayout.Height(28)))
                InstallAndExport();
            EditorGUI.BeginDisabledGroup(!ExpressionInstall.HasBackup());
            if (GUILayout.Button("Restore backup / 元に戻す", GUILayout.Height(24)))
                RestoreBackup();
            EditorGUI.EndDisabledGroup();
        }

        void DrawCandidate(int index)
        {
            var item = _found[index];
            GUILayout.BeginVertical("box");
            GUILayout.BeginHorizontal();
            if (item.thumb != null)
                GUILayout.Label(item.thumb, GUILayout.Width(72), GUILayout.Height(72));
            else
                GUILayout.Label(item.include ? "(no preview)" : "", GUILayout.Width(72), GUILayout.Height(72));
            GUILayout.BeginVertical();
            item.displayName = EditorGUILayout.TextField("Name", item.displayName);
            if (item.include)
            {
                int next = EditorGUILayout.IntPopup(
                    "Slot",
                    item.slot,
                    new[] { "(none)", "Shift+F1", "Shift+F2", "Shift+F3", "Shift+F4" },
                    new[] { 0, 1, 2, 3, 4 });
                if (next != item.slot)
                    AssignSlot(item, next);
                if (GUILayout.Button("Exclude / 除外", GUILayout.Width(140)))
                {
                    item.include = false;
                    AssignSlot(item, 0);
                }
            }
            else
            {
                EditorGUILayout.LabelField("Excluded", item.reason);
                if (GUILayout.Button("Restore / 戻す", GUILayout.Width(140)))
                {
                    item.include = true;
                    RenderOne(item);
                }
            }
            GUILayout.EndVertical();
            GUILayout.EndHorizontal();
            GUILayout.EndVertical();
        }

        void AssignSlot(Candidate item, int slot)
        {
            if (slot > 0)
            {
                for (int i = 0; i < _found.Count; i++)
                {
                    if (_found[i] != item && _found[i].slot == slot)
                        _found[i].slot = 0;
                }
            }
            item.slot = slot;
        }

        void PullId()
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

        void FindExpressions()
        {
            if (_avatar == null)
            {
                EditorUtility.DisplayDialog("Expression Profile", "Assign an avatar first.", "OK");
                return;
            }
            PullId();
            ClearThumbs();
            _found.Clear();
            _scan = ExpressionScan.Gather(_avatar);
            int included = 0;
            int slot = 1;
            try
            {
                for (int i = 0; i < _scan.entries.Count; i++)
                {
                    var entry = _scan.entries[i];
                    var candidate = new Candidate
                    {
                        clip = entry.clip,
                        displayName = entry.clip != null ? entry.clip.name : entry.verdict.name,
                        kind = entry.verdict.kind,
                        reason = entry.verdict.reason,
                        suggested = entry.verdict.include,
                        include = entry.verdict.include
                    };
                    if (candidate.include && slot <= ProfileDocument.SlotCount)
                    {
                        candidate.slot = slot;
                        slot++;
                        included++;
                    }
                    _found.Add(candidate);
                }
                for (int i = 0; i < _found.Count; i++)
                {
                    if (_found[i].include)
                        RenderOne(_found[i]);
                }
            }
            finally
            {
                EditorUtility.ClearProgressBar();
            }
            string bits = _scan.syncedBits.HasValue ? _scan.syncedBits.Value.ToString() : "unknown";
            string wd = _scan.writeDefaults ? "on" : "off";
            _status = included + " expressions, " + (_found.Count - included) + " excluded. "
                + "Write Defaults " + wd + ". Synced bits " + bits
                + " (4 bools need 4 more, limit " + ExpressionRules.SyncLimit + ")."
                + (VrcOptional.SdkCompiled ? " VRChat SDK detected." : "")
                + (string.IsNullOrEmpty(_scan.notice) ? "" : " " + _scan.notice);
        }

        void RenderOne(Candidate item)
        {
            if (_avatar == null || item.clip == null)
                return;
            string folder = Path.Combine(Path.GetTempPath(), "VREmoKeyThumbs");
            Directory.CreateDirectory(folder);
            string path = Path.Combine(folder, ProfileDocument.Sanitize(item.clip.name) + ".png");
            var pose = ExpressionPreview.Capture(_avatar);
            bool wrote = false;
            try
            {
                wrote = ExpressionPreview.TryRender(_avatar, item.clip, path, 256);
            }
            finally
            {
                ExpressionPreview.Restore(pose);
            }
            if (!wrote)
                return;
            if (item.thumb != null)
                DestroyImmediate(item.thumb);
            var image = new Texture2D(2, 2, TextureFormat.RGB24, false);
            if (!image.LoadImage(File.ReadAllBytes(path)))
            {
                DestroyImmediate(image);
                return;
            }
            item.thumb = image;
            item.thumbPath = path;
        }

        void InstallAndExport()
        {
            if (_scan == null || _scan.controller == null)
            {
                EditorUtility.DisplayDialog("Expression Profile", "Find expressions on an avatar with an FX controller first.", "OK");
                return;
            }
            if (string.IsNullOrEmpty(_avatarId))
                PullId();
            if (string.IsNullOrEmpty(ProfileDocument.Sanitize(_avatarId)))
            {
                EditorUtility.DisplayDialog("Expression Profile", "Enter an avatar ID.", "OK");
                return;
            }
            var chosen = Chosen();
            if (chosen.Count == 0)
            {
                EditorUtility.DisplayDialog("Expression Profile", "Assign at least one expression to Shift+F1..F4.", "OK");
                return;
            }
            string folder = EditorUtility.SaveFolderPanel(
                "Export profile folder", "", ProfileDocument.Sanitize(_avatarId));
            if (string.IsNullOrEmpty(folder))
                return;
            var request = new ExpressionInstall.Request
            {
                controller = _scan.controller,
                expressionParameters = _scan.expressionParameters,
                controllerFact = _scan.controllerFact,
                expressionFacts = _scan.expressionFacts
            };
            for (int i = 0; i < chosen.Count; i++)
            {
                request.names.Add(string.IsNullOrEmpty(chosen[i].displayName) ? chosen[i].clip.name : chosen[i].displayName);
                request.clips.Add(chosen[i].clip);
                request.slots.Add(chosen[i].slot);
            }
            var plan = ExpressionInstall.Apply(request);
            if (!plan.ok)
            {
                EditorUtility.DisplayDialog("Expression Profile", plan.error, "OK");
                return;
            }
            var rows = new List<ProfileDocument.Row>();
            Directory.CreateDirectory(Path.Combine(folder, "thumbs"));
            var pose = _avatar != null ? ExpressionPreview.Capture(_avatar) : null;
            try
            {
                for (int i = 0; i < chosen.Count; i++)
                {
                    int index = chosen[i].slot;
                    string parameter = ExpressionRules.ParameterName(index);
                    var row = new ProfileDocument.Row
                    {
                        index = index,
                        name = request.names[i],
                        parameter = parameter,
                        osc = ExpressionRules.OscAddress(parameter),
                        clip = chosen[i].clip
                    };
                    string png = Path.Combine(folder, "thumbs", index + ".png");
                    bool wrote = _avatar != null && chosen[i].clip != null
                        && ExpressionPreview.TryRender(_avatar, chosen[i].clip, png, 256);
                    if (!wrote && !string.IsNullOrEmpty(chosen[i].thumbPath) && File.Exists(chosen[i].thumbPath))
                    {
                        File.Copy(chosen[i].thumbPath, png, true);
                        wrote = true;
                    }
                    if (wrote)
                        row.thumb = "thumbs/" + index + ".png";
                    rows.Add(row);
                }
            }
            finally
            {
                if (pose != null)
                    ExpressionPreview.Restore(pose);
            }
            string json = ProfileDocument.Build(
                ProfileDocument.Sanitize(_avatarId), _avatarName, _avatarId, rows);
            File.WriteAllText(Path.Combine(folder, "profile.json"), json);
            _scan.controllerFact = ExpressionScan.Describe(_scan.controller);
            _scan.expressionFacts = ExpressionScan.DescribeParameters(_scan.expressionParameters);
            _scan.writeDefaults = _scan.controllerFact.writeOn > _scan.controllerFact.writeOff;
            EditorUtility.DisplayDialog(
                "Expression Profile",
                "Installed " + plan.layers.Count + " layer(s) and wrote " + folder
                + "\nImport that folder (or a zip of it) in the desktop app.",
                "OK");
        }

        List<Candidate> Chosen()
        {
            var chosen = new List<Candidate>();
            for (int slot = 1; slot <= ProfileDocument.SlotCount; slot++)
            {
                for (int i = 0; i < _found.Count; i++)
                {
                    if (_found[i].include && _found[i].slot == slot && _found[i].clip != null)
                    {
                        chosen.Add(_found[i]);
                        break;
                    }
                }
            }
            return chosen;
        }

        void RestoreBackup()
        {
            int count = ExpressionInstall.Restore();
            EditorUtility.DisplayDialog(
                "Expression Profile",
                count > 0 ? "Restored " + count + " asset(s) from VREmoKeyBackup." : "No backup was found.",
                "OK");
        }

        void ClearThumbs()
        {
            for (int i = 0; i < _found.Count; i++)
            {
                if (_found[i].thumb != null)
                    DestroyImmediate(_found[i].thumb);
                _found[i].thumb = null;
            }
        }
    }

    public static class ProfileDocument
    {
        public const int SlotCount = 4;

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
                    if (row == null || row.index < 1 || row.index > SlotCount
                        || string.IsNullOrEmpty(row.name))
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
        public class Pose
        {
            public Transform root;
            public Vector3 position;
            public Quaternion rotation;
            public Vector3 scale;
            public bool animatorEnabled;
            public Animator animator;
            public readonly List<SkinnedMeshRenderer> renderers = new List<SkinnedMeshRenderer>();
            public readonly List<float[]> weights = new List<float[]>();
        }

        public static Pose Capture(GameObject avatar)
        {
            var pose = new Pose();
            if (avatar == null)
                return pose;
            pose.root = avatar.transform;
            pose.position = avatar.transform.localPosition;
            pose.rotation = avatar.transform.localRotation;
            pose.scale = avatar.transform.localScale;
            pose.animator = avatar.GetComponentInChildren<Animator>();
            if (pose.animator != null)
                pose.animatorEnabled = pose.animator.enabled;
            foreach (var renderer in avatar.GetComponentsInChildren<SkinnedMeshRenderer>(true))
            {
                int count = renderer.sharedMesh != null ? renderer.sharedMesh.blendShapeCount : 0;
                var values = new float[count];
                for (int i = 0; i < count; i++)
                    values[i] = renderer.GetBlendShapeWeight(i);
                pose.renderers.Add(renderer);
                pose.weights.Add(values);
            }
            return pose;
        }

        public static void Restore(Pose pose)
        {
            if (pose == null)
                return;
            for (int i = 0; i < pose.renderers.Count; i++)
            {
                var renderer = pose.renderers[i];
                if (renderer == null)
                    continue;
                var values = pose.weights[i];
                int count = renderer.sharedMesh != null ? renderer.sharedMesh.blendShapeCount : 0;
                int limit = Mathf.Min(count, values.Length);
                for (int shape = 0; shape < limit; shape++)
                    renderer.SetBlendShapeWeight(shape, values[shape]);
            }
            if (pose.animator != null)
                pose.animator.enabled = pose.animatorEnabled;
            if (pose.root != null)
            {
                pose.root.localPosition = pose.position;
                pose.root.localRotation = pose.rotation;
                pose.root.localScale = pose.scale;
            }
        }

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
