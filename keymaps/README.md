# 純正Cornixの配列資料

初期配列はメーカー公開の10レイヤーを基準にしています。PCへのUSBは右が担当するため、CoreXでは左右の行列・ダイヤル番号を交換しています。純正ファームで保存した `.vil` をそのまま読み込まないでください。設定変更と新しいバックアップにはStudioまたはVialを使います。

## 基準にしたファイル

2026-10-09に[メーカーの日本語マニュアル](https://docs.channel.io/jezailfunderjp/ja/articles/Cornix-%E6%97%A5%E6%9C%AC%E8%AA%9E%E3%83%9E%E3%83%8B%E3%83%A5%E3%82%A2%E3%83%AB-c1160246)の「初期キーマップ」から取得した `cornix-default-keymap.vil` を使っています。ファームの版番号は付いていません。

- [メーカー配布の原本](https://cf.channel.io/document/spaces/16010/articles/541498/revisions/942375/usermedia/6958e9940227a7198545)
- [照合用に保存した原本](reference/cornix-default-keymap.vil)
- 原本のSHA-256：`f85dd13d58398ea53e29f3fbab88d07b1f86ba09982e7eaac5d36eb314a327ab`


`tools/stock_cornix.py` が純正用設定を生成します。同フォルダのCoreX用プリセットは分離元の回帰テスト資料として保持しており、純正Cornixへ読み込むものではありません。
