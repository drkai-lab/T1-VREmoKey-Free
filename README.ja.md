# VRChat 表情ホットキー

[English](README.md) | 日本語

無料版です。Shift を押しながら F1 から F4 で VRChat の表情を切り替えます。
表情は 4 つまでです。アバターごとに名前とプレビュー画像を持てます。この
リポジトリはキーボードハードウェアと通信せず、ファームウェアも書きません。

T1 ミニキーボード向けの完全版は別のアプリケーションです。あちらは表情 12、
キーボード本体のボタン、およびダイヤルに対応します。このプロジェクトには
含まれません。

## できること

1. Unity で `Tools > VRChat Expression Hotkeys > Avatar Profile` を開きます。
   表情に名前を付け、256×256 の顔プレビューを書き出し（アバターを開いている
   シーンが必要です）、`profile.json` と `thumbs/*.png` のフォルダを書き出します。
   エディタツールはリフレクションだけを使うので、VRChat SDK が無いプロジェクト
   でもコンパイルできます。一つのファイルにしたいときはそのフォルダを zip します。
2. このアプリでパックを読み込みます。ドロップダウンで選ぶか、VRChat に
   切り替えさせます。VRChat は既定で `/avatar/change` を `127.0.0.1:9001` へ
   送ります。待受ポートは変更できます。ポートが使用中でもアプリは動き続け、
   その旨を表示します。OSC ルーターで `/avatar/change` を選んだポートへ転送できます。
3. F1..F4 のそれぞれに表情を割り当てます。行には番号、アバター上の名前、
   サムネイルが出ます。PNG が無いときは汎用アイコンと `Expression N` です。
   Shift+F5..F12 は何もしません。
4. アプリを起動したままにします。Shift+Fn はその表情の OSC bool を送ります。
   押したとき TRUE、離したとき FALSE。キーリピートは無視します。既定では
   VRChat が前面のときだけ送ります。

組み込みの Default プロファイルは Expression1..4 です。読み込んだ
プロファイルに 5 番以降があっても、それらは無視され、その旨が表示されます。

コードの待受は Windows（キーを飲み込まない低レベルキーボードフック）と
Linux（デバイスをグラブしない evdev）です。macOS ではプロファイルと割り当てを
編集できますが、キー待受はありません。

Linux でキーボードが読めないときは、ユーザーを `input` グループに入れて
ログインし直してください。キーはデスクトップへそのまま届きます。

## ダウンロード

最新版は [リリースページ](https://github.com/drkai-lab/vrc-expression-hotkeys/releases/latest) から入手できます。

| OS | ファイル |
| --- | --- |
| Windows（おすすめ） | `vrc-expression-hotkeys-<version>-setup.exe`（インストーラ） |
| Windows（インストール不要） | `vrc-expression-hotkeys-gui.exe`（ウィンドウ版）/ `vrc-expression-hotkeys.exe`（コンソール版） |
| Linux | `vrc-expression-hotkeys-linux-<version>.tar.gz`（と `.sha256`） |

## Windows へのインストール（インストーラ）

1. [リリースページ](https://github.com/drkai-lab/vrc-expression-hotkeys/releases/latest) から `vrc-expression-hotkeys-<version>-setup.exe` をダウンロードして実行します。
2. 署名していないため SmartScreen の警告が出ることがあります。「**詳細情報**」→「**実行**」で続行してください。
3. セットアップ中に選べる項目（どちらも既定はオフ）:
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

設定はアプリの設定ディレクトリにあります（Linux は
`~/.config/vrc-expression-hotkeys`、Windows は
`%APPDATA%\vrc-expression-hotkeys`）。プロファイルはそこにある
`avatars/<id>/profile.json` と `thumbs/*.png` です。

## 手動チェックリスト

- [ ] Unity からプロファイルを書き出し、このアプリで読み込む。F1..F4 に名前とサムネイルが出る。
- [ ] 5 番以降の表情を含むプロファイルを読み込む。それらは飛ばされ、通知が出る。Shift+F5 は送られない。
- [ ] F3 を割り当て、VRChat を前面にして Shift+F3 を押す。表情がオンになり、離すと false。
- [ ] 別のアプリが前面のとき、同じコードは送られない。
- [ ] 待受ポートへアバター ID 入りの `/avatar/change` を送るとドロップダウンが切り替わる。
- [ ] 待受ポートが使用中のとき、使用中と出てウィンドウは開いたまま。
- [ ] PNG の無い表情は Expression N と出る。

## Unity パッケージ

[unity/README.md](unity/README.md) を見てください。`unity/` をプロジェクトの
`Packages` にコピーするか、ローカルパッケージとして追加します。

## ライセンス

MIT。[LICENSE](LICENSE) を見てください。
