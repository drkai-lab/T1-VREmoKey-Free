# T1-VREmoKey-Free

VRChat 表情ホットキー（T1-VREmoKey の無料版）。

[English](README.md) | 日本語

無料版では、Shift を押しながら F1 から F4 を押して VRChat の表情を切り替えます。扱える表情は 4 つまでで、アバターごとに名前とプレビュー画像を設定できます。このリポジトリはキーボードハードウェアと通信せず、ファームウェアも書き込みません。

完全版の T1-VREmoKey は、T1 ミニキーボード向けの別プロジェクトです。完全版は 12 個の表情、キーボード本体のボタンへの割り当て、ダイヤルでの表情の切り替え、キーボードへの設定の書き込みに対応しています。

アプリ本体・ファイル・コマンドの名前は `vrc-expression-hotkeys` のままです（スタートメニューでは VRChat Expression Hotkeys）。既存の設定はそのまま使えます。

## できること

1. Unity で `Tools > VRChat Expression Hotkeys > Avatar Profile` を開きます。
   表情に名前を付けて 256×256 の顔プレビューを生成し（アバターを開いたシーンが必要です）、`profile.json` と `thumbs/*.png` を含むフォルダを書き出します。
   エディタツールはリフレクションだけを使うので、VRChat SDK が無いプロジェクトでもコンパイルできます。1 つのファイルにまとめたいときは、そのフォルダを zip にします。
2. このアプリでパックを読み込みます。アバターはドロップダウンで選ぶか、VRChat に切り替えを任せます。
   VRChat は既定で `/avatar/change` を `127.0.0.1:9001` へ送ります。待受ポートは変更できます。ポートが使用中でもアプリは動き続け、使用中であることを表示します。OSC ルーターを使うと、`/avatar/change` を選んだポートへ転送できます。
3. F1..F4 のそれぞれに表情を割り当てます。各行には番号、アバター上の名前、サムネイルが表示されます。PNG が無いときは、汎用アイコンと `Expression N` が表示されます。
   Shift+F5..F12 は何もしません。
4. アプリは起動したままにしておきます。Shift+Fn を押すと、アプリがその表情の OSC bool を送ります。押したときに TRUE、離したときに FALSE を送り、キーリピートは無視します。既定では、VRChat が前面にあるときだけ送ります。

組み込みの Default プロファイルは Expression1..4 です。読み込んだプロファイルに 5 番以降の表情が含まれていても、それらは無視され、その旨が表示されます。

キーの組み合わせの待受は、Windows（キーを飲み込まない低レベルキーボードフック）と Linux（デバイスをグラブしない evdev）で動きます。macOS ではプロファイルと割り当てを編集できますが、キーの待受には対応していません。

Linux でキーボードが読めないときは、ユーザーを `input` グループに入れてログインし直してください。キー入力はそのままデスクトップに届きます。

## ダウンロード

最新版は [リリースページ](https://github.com/drkai-lab/T1-VREmoKey-Free/releases/latest) から入手できます。

| OS | ファイル |
| --- | --- |
| Windows（おすすめ） | `vrc-expression-hotkeys-<version>-setup.exe`（インストーラ） |
| Windows（インストール不要） | `vrc-expression-hotkeys-gui.exe`（ウィンドウ版）/ `vrc-expression-hotkeys.exe`（コンソール版） |
| Linux | `vrc-expression-hotkeys-linux-<version>.tar.gz`（と `.sha256`） |

## Windows へのインストール（インストーラ）

1. [リリースページ](https://github.com/drkai-lab/T1-VREmoKey-Free/releases/latest) から `vrc-expression-hotkeys-<version>-setup.exe` をダウンロードして実行します。
2. 署名していないため SmartScreen の警告が出ることがあります。「**詳細情報**」→「**実行**」で続行してください。
3. セットアップ中に次の項目を選べます（どちらも既定はオフです）。
   - **デスクトップにアイコンを作成**
   - **Start at logon**（ログオン時に起動）: Windows にサインインしたとき自動で起動します。
4. スタートメニューの **VRChat Expression Hotkeys** から起動します。遊んでいる間はウィンドウを開いたまま（最小化でも可）にしてください。
5. アンインストールは「**設定 > アプリ > インストールされているアプリ**」か、スタートメニューのアンインストール用ショートカットから行えます。

インストールせずに使う場合は、`vrc-expression-hotkeys-gui.exe` をダウンロードしてそのまま実行します。

## Linux へのインストール

必要なもの: Python 3.9 以上と Tkinter（Arch / Omarchy は `sudo pacman -S tk`、Debian / Ubuntu は `sudo apt install python3-tk`）。

```bash
# 1. リリースページから tar.gz と .sha256 をダウンロードし、検証して展開
sha256sum -c vrc-expression-hotkeys-linux-<version>.tar.gz.sha256
tar xzf vrc-expression-hotkeys-linux-<version>.tar.gz
cd vrc-expression-hotkeys-linux-<version>

# 2. PATH の通った場所に配置
install -Dm755 vrc-expression-hotkeys.py ~/.local/bin/vrc-expression-hotkeys

# 3. キーボードを読めるようにする（実行後にログアウトして入り直す）
sudo usermod -aG input "$USER"

# 4. 起動
vrc-expression-hotkeys            # ウィンドウ
vrc-expression-hotkeys --listen   # ウィンドウなしで待受だけ
```

任意: ログイン時に待受を自動起動する（systemd ユーザーサービス）。

```bash
mkdir -p ~/.config/systemd/user
cat > ~/.config/systemd/user/vrc-expression-hotkeys.service <<'EOF'
[Unit]
Description=VRChat Expression Hotkeys (Shift+F1..F4)

[Service]
ExecStart=%h/.local/bin/vrc-expression-hotkeys --listen
Restart=on-failure

[Install]
WantedBy=default.target
EOF
systemctl --user daemon-reload
systemctl --user enable --now vrc-expression-hotkeys.service
```

VRChat が前面かどうかは、Wayland（Hyprland / Omarchy）では `hyprctl`、X11 では `xdotool` か `xprop` で判定します。

アンインストール: `systemctl --user disable --now vrc-expression-hotkeys.service` を実行し、`~/.local/bin/vrc-expression-hotkeys` と `~/.config/systemd/user/vrc-expression-hotkeys.service` を削除します。

## チェックアウトから実行

```bash
python3 vrc-expression-hotkeys.py
python3 vrc-expression-hotkeys.py --listen
```

設定はアプリの設定ディレクトリにあります（Linux は `~/.config/vrc-expression-hotkeys`、Windows は `%APPDATA%\vrc-expression-hotkeys`）。プロファイルは、その中にある `avatars/<id>/profile.json` と `thumbs/*.png` です。

## 手動チェックリスト

- [ ] Unity からプロファイルを書き出し、このアプリで読み込む。F1..F4 に名前とサムネイルが出る。
- [ ] 5 番以降の表情を含むプロファイルを読み込む。それらは飛ばされ、通知が出る。Shift+F5 は送られない。
- [ ] F3 を割り当て、VRChat を前面にして Shift+F3 を押す。表情がオンになり、離すと false。
- [ ] 別のアプリが前面のときは、同じキー操作をしても送られない。
- [ ] 待受ポートへアバター ID 入りの `/avatar/change` を送るとドロップダウンが切り替わる。
- [ ] 待受ポートが使用中のとき、使用中と出てウィンドウは開いたまま。
- [ ] PNG の無い表情は Expression N と出る。

## Unity パッケージ

[unity/README.md](unity/README.md) を見てください。`unity/` をプロジェクトの `Packages` にコピーするか、ローカルパッケージとして追加します。

## ライセンス

MIT。[LICENSE](LICENSE) を見てください。
