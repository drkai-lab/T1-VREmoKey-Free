# VRChat 表情ホットキー

無料版です。Shift を押しながら F1 から F12 で VRChat の表情を切り替えます。
アバターごとに名前とプレビュー画像を持てます。このリポジトリはキーボード
ハードウェアと通信せず、ファームウェアも書きません。

T1 ミニキーボード向けの完全版は別のアプリケーションです。あちらはキーボード
本体のキーとダイヤルへ表情を割り当てます。このプロジェクトには含まれません。

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
3. F1..F12 のそれぞれに表情を割り当てます。行には番号、アバター上の名前、
   サムネイルが出ます。PNG が無いときは汎用アイコンと `Expression N` です。
4. アプリを起動したままにします。Shift+Fn はその表情の OSC bool を送ります。
   押したとき TRUE、離したとき FALSE。キーリピートは無視します。既定では
   VRChat が前面のときだけ送ります。

組み込みの Default プロファイルは、一般的な名前の Expression1..12 です。

コードの待受は Windows（キーを飲み込まない低レベルキーボードフック）と
Linux（デバイスをグラブしない evdev）です。macOS ではプロファイルと割り当てを
編集できますが、キー待受はありません。

Linux でキーボードが読めないときは、ユーザーを `input` グループに入れて
ログインし直してください。キーはデスクトップへそのまま届きます。

## ダウンロード

Windows は、リリースが公開されたあとのインストーラ、またはコンソール版と
ウィンドウ版の実行ファイルです。

Linux:

```bash
tar xzf vrc-expression-hotkeys-linux-1.0.0.tar.gz
cd vrc-expression-hotkeys-linux-1.0.0
python3 vrc-expression-hotkeys.py
```

チェックアウトから:

```bash
python3 vrc-expression-hotkeys.py
python3 vrc-expression-hotkeys.py --listen
```

設定はアプリの設定ディレクトリにあります（Linux は
`~/.config/vrc-expression-hotkeys`、Windows は
`%APPDATA%\vrc-expression-hotkeys`）。プロファイルはそこにある
`avatars/<id>/profile.json` と `thumbs/*.png` です。

## 手動チェックリスト

- [ ] Unity からプロファイルを書き出し、このアプリで読み込む。F1..F12 に名前とサムネイルが出る。
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
