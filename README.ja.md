# T1-VREmoKey-Free

VRChat 表情ホットキー（T1-VREmoKey の無料版）。

[English](README.md) | 日本語

無料版では、Shift を押しながら F1 から F4 を押して VRChat の表情を切り替えます。扱える表情は 4 つまでで、アバターごとに名前とプレビュー画像を設定できます。このリポジトリはキーボードハードウェアと通信せず、ファームウェアも書き込みません。

完全版の T1-VREmoKey は、T1 ミニキーボード向けの別プロジェクトです。完全版は 12 個の表情、キーボード本体のボタンへの割り当て、ダイヤルでの表情の切り替え、キーボードへの設定の書き込みに対応しています。

コマンド名は `vrc-expression-hotkeys` のまま、設定の保存場所も変わりません。今までの設定をそのまま使えます。バージョン 2.0.2 は Rust 版で、ウィンドウとトレイに出る名前は **T1-VREmoKey-Free** です。リリースビルドはシンボルを削り、リンク時最適化を掛けています。難読化したビルドではありません。

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

ウィンドウを閉じてもアプリはシステムトレイに残ります。メニューは「画面を開く」「待受の開始」「待受の停止」「終了」です。表示言語が英語のときは Open window、Start listening、Stop listening、Quit になります。アプリを終えるのは「終了」です。

## ダウンロード

最新版は [リリースページ](https://github.com/drkai-lab/T1-VREmoKey-Free/releases/latest) から入手できます。

| OS | ファイル |
| --- | --- |
| Windows（おすすめ） | `vrc-expression-hotkeys-<version>-setup.exe`（インストーラ） |
| Windows（インストール不要） | `vrc-expression-hotkeys-gui.exe`（ウィンドウ版）/ `vrc-expression-hotkeys.exe`（コンソール版） |
| Debian / Ubuntu | `vrc-expression-hotkeys_<version>-1_amd64.deb` |
| Arch | `vrc-expression-hotkeys-<version>-1-x86_64.pkg.tar.zst` |
| Linux（tar アーカイブ） | `vrc-expression-hotkeys-linux-<version>.tar.gz`（と `.sha256`） |
| Unity | `com.drkai-lab.vrc-expression-hotkeys-<version>.tgz` |

Windows の実行ファイルは Visual C++ 再頒布可能パッケージなしで動きます。

## Windows へのインストール（インストーラ）

1. [リリースページ](https://github.com/drkai-lab/T1-VREmoKey-Free/releases/latest) から `vrc-expression-hotkeys-<version>-setup.exe` をダウンロードして実行します。
2. 署名していないため SmartScreen の警告が出ることがあります。「**詳細情報**」→「**実行**」で続行してください。
3. セットアップ中に次の項目を選べます（どちらも既定はオフです）。
   - **デスクトップにアイコンを作成**
   - **Start at logon**（ログオン時に起動）: Windows にサインインしたとき自動で起動します。
4. スタートメニューの **T1-VREmoKey-Free** から起動します。ウィンドウは閉じてもトレイに残ります。終了するときはメニューの「終了」を選びます。
5. アンインストールは「**設定 > アプリ > インストールされているアプリ**」か、スタートメニューのアンインストール用ショートカットから行えます。

インストールせずに使う場合は、`vrc-expression-hotkeys-gui.exe` をダウンロードしてそのまま実行します。

## Linux へのインストール

VRChat が前面かどうかは、Wayland（Hyprland / Omarchy）では `hyprctl`、X11 では `xdotool` か `xprop` で判定します。キーボードを読むには `input` グループが必要です（`sudo usermod -aG input "$USER"` のあと、ログインし直します）。パッケージには入力デバイス用のルールも入っています。

Debian または Ubuntu:

```bash
sudo apt install ./vrc-expression-hotkeys_<version>-1_amd64.deb
vrc-expression-hotkeys-gui
vrc-expression-hotkeys --listen
```

Arch Linux は、リリースページのパッケージを入れます（`rust/packaging/` に `PKGBUILD` もあります）。

```bash
sudo pacman -U vrc-expression-hotkeys-<version>-1-x86_64.pkg.tar.zst
```

tar アーカイブから入れる場合:

```bash
sha256sum -c vrc-expression-hotkeys-linux-<version>.tar.gz.sha256
tar xzf vrc-expression-hotkeys-linux-<version>.tar.gz
cd vrc-expression-hotkeys-linux-<version>
./install.sh                  # ~/.local に入れる
./install.sh --autostart      # ログイン時に、ウィンドウなしの待受を起動
./install.sh --autostart-gui  # ログイン時にウィンドウを起動
```

`--autostart` と `--autostart-gui` は同時には使えません。前者は systemd のユーザーサービス、後者は XDG の自動起動です。外すときは `./uninstall.sh` を実行します。

`.deb` を入れたあと、同じ待受を有効にするには `systemctl --user enable --now vrc-expression-hotkeys.service` です。

## チェックアウトから実行

```bash
cd rust
cargo run --release --bin vrc-expression-hotkeys-gui
cargo run --release --bin vrc-expression-hotkeys -- --listen
```

Python 1.0 のスクリプトもリポジトリに残してあります。

```bash
python3 vrc-expression-hotkeys.py
python3 vrc-expression-hotkeys.py --listen
```

設定はアプリの設定ディレクトリにあります（Linux は `~/.config/vrc-expression-hotkeys`、Windows は `%APPDATA%\vrc-expression-hotkeys`）。プロファイルは、その中にある `avatars/<id>/profile.json` と `thumbs/*.png` です。

## 実機なしの通しテスト

無料版はキーボード本体と通信しません。T1 のボタンは、ここでは Shift+F1 から Shift+F4 を押して離すキーとしてだけ扱います。テスト用ビルドは、そのキーをアプリのキーフックの下に差し込めます。Linux では evdev の `poll`、Windows では低レベルキーボードフックの `poll` が、本物のキーと同じ形に変えます。偽の VRChat は OSC の bool を記録し、`/avatar/change` を送れます。

この差し込みは配布物に入りません。`cargo build --release` と各パッケージは `--features emulate` を付けません。公開バイナリに `--emulate` は無く、注入用のキューもありません。

```bash
cd rust
cargo test --lib walkthrough_configure_confirm_use_and_switch_avatar
cargo run --features emulate --bin vrc-expression-hotkeys -- --emulate
```

`cargo test` だけでも、ライブラリテストの一つとして同じ通しが走ります。`--emulate` はコンソール版の中で同じ通しを実行し、全部通ると終了コード 0 で終わります。

Windows 向けに feature を付けてビルドした exe は、Wine でも同じコマンドで走ります。

```bash
cargo build --target x86_64-pc-windows-gnu --features emulate --bin vrc-expression-hotkeys
wine target/x86_64-pc-windows-gnu/debug/vrc-expression-hotkeys.exe --emulate
```

通しの中身は次のとおりです。

1. 本番のプロファイル取り込みで、表情の違うアバターを 2 つ読み、`settings.json` を書きます。
2. 保存結果を読み直し、F1 が最初のアバターのパラメータになることを確認します。
3. 偽 T1 が Shift+F1 を押して離します。偽 VRChat は true のあと false を記録します。F1 のアドレスは `/avatar/parameters/GrinA`（24 バイト）です。通しは、このパケットがアドレスのあとに NUL を 4 バイト足し、型タグ `,T` も 4 バイト境界まで NUL で埋めていることを確認します。
4. フォーカスの差し替えを「前面ではない」にして同じコードを送り、OSC が出ないことを確認します。
5. 差し替えを外し、本番の前面判定を使います。コードを送るのは、その判定が VRChat を前面と見たときだけです。画面の無い環境では前面ウィンドウの名前が取れないことが多く、`when_unknown` は `deny` のままなので、そのコードは送られません。
6. 偽 VRChat が `/avatar/change` で 2 体目へ切り替えます。保存されたアバターを確認したあと、Shift+F1 は 2 体目のパラメータになります。

通しのあいだ、`127.0.0.1` の注入ポートが `press N` と `release N`（N は 1 から 4）、それに `focus on` / `focus off` / `focus live` を受け付けます。最後のコードは、フックのスレッドの外からこのポート経由で送ります。

限界:

- 偽 T1 はデバイスノードを作りません。注入イベントは `/dev/input` も、物理的な `WH_KEYBOARD_LL` のパケットも通りません。Wine では実キーの低レベルフックが届かないことが多いです。このコマンドはその配送を証明しません。レポートの `hook: low-level installed` または `hook: low-level unavailable` が、`SetWindowsHookExW` の成否です。
- 通しのあいだ、フックは物理キーボードを読みません。そのマシンでの打鍵が結果を変えないようにするためです。実キーがフックに届くことは見ません。
- 対象は Shift+F1..F4 の押下と解放で、一度に一つのコードです。F5..F12、ダイヤル、ハードウェアのレポートは範囲外です。F5 を混ぜても OSC が出ないことだけ確認します。
- 偽 VRChat が記録するのは OSC bool で、送れるのは `/avatar/change` の文字列です。アバターのロード、アニメータ、bool 以外のパラメータは見ません。
- フォーカスの差し替え口は、このテスト用ビルドにだけあります。外したときは Windows の前面プロセス判定か、Linux の `hyprctl` / `xdotool` / `xprop` が走ります。この段階が通るのは、本番の判定が走り、ゲートがその結果に従ったときです。
- ウィンドウ、トレイ、Unity からの書き出しは、このコマンドでは操作しません。
- `--emulate` は一時ディレクトリだけを使います。`~/.config/vrc-expression-hotkeys` と `%APPDATA%\vrc-expression-hotkeys` は読み書きしません。

## 手動チェックリスト

- [ ] Unity からプロファイルを書き出し、このアプリで読み込む。F1..F4 に名前とサムネイルが出る。
- [ ] 5 番以降の表情を含むプロファイルを読み込む。それらは飛ばされ、通知が出る。Shift+F5 は送られない。
- [ ] F3 を割り当て、VRChat を前面にして Shift+F3 を押す。表情がオンになり、離すと false。
- [ ] 別のアプリが前面のときは、同じキー操作をしても送られない。
- [ ] 待受ポートへアバター ID 入りの `/avatar/change` を送るとドロップダウンが切り替わる。
- [ ] 待受ポートが使用中のとき、使用中と出てウィンドウは開いたまま。
- [ ] PNG の無い表情は Expression N と出る。

## Unity パッケージ

エディタ用のツールは `unity/` にあります（F1 から F4 まで）。入れ方は次の 2 つです。詳しくは [unity/README.md](unity/README.md) も見てください。

### A. リリースの tarball から

1. リリースページから `com.drkai-lab.vrc-expression-hotkeys-<ver>.tgz` をダウンロードします。
2. Unity で **Window > Package Manager** を開きます。
3. **+** をクリックします。
4. **Add package from tarball...** を選びます。
5. `com.drkai-lab.vrc-expression-hotkeys-<ver>.tgz` を指定します。

### B. Git から

Git がインストールされている必要があります。

1. **Window > Package Manager** を開きます。
2. **+** をクリックします。
3. **Add package from git URL...** を選びます。
4. `https://github.com/drkai-lab/T1-VREmoKey-Free.git?path=unity` を入力します。
5. **Add** をクリックします。

版を固定したいときは、末尾に `#v2.0.2` を足します。

`https://github.com/drkai-lab/T1-VREmoKey-Free.git?path=unity#v2.0.2`

## ライセンス

MIT。[LICENSE](LICENSE) を見てください。

ウィンドウの日本語は、同梱の [Noto Sans JP](https://fonts.google.com/noto/specimen/Noto+Sans+JP) Regular（SIL Open Font License 1.1）で描きます。表記は [rust/assets/OFL](rust/assets/OFL) で、インストーラはプログラムの横にこのファイルを置きます。トレイメニューとファイルダイアログは OS の部品です。Windows では文字を UTF-16 で渡し、表示にはシステムの UI フォントを使います。
