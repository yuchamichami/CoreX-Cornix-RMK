# CoreX for Cornix

Cornix純正の左右基板でCoreXファームを使うためのリポジトリです。純正の50キー（ダイヤル押し込み2個を含む）・10レイヤー・左右ダイヤルを保持し、[CoreX Studio](https://corex-studio.pages.dev/)とVialで設定できます。PCへのUSB接続は右側、Bluetooth名は **Cornix CoreX** です。

**現在は実機確認前の試験版です。** ビルド・通信モック・配列と書き込み範囲を検証していますが、純正基板での起動、入力、左右接続、LED、電池消費は未確認です。

## ダウンロードと使い方

[配布ファイル](firmware/)から基板に合うUF2を選び、[導入手順](docs/cornix-stock.md)に従って左右とも書き込みます。

| 基板 | ファイル |
| --- | --- |
| Cornix純正右 | [Cornix-Stock-Right-v0.11.4.uf2](firmware/Cornix-Stock-Right-v0.11.4.uf2) |
| Cornix純正左 | [Cornix-Stock-Left-v0.11.4.uf2](firmware/Cornix-Stock-Left-v0.11.4.uf2) |

メーカー配布のCornix LP V1.12と公開基板定義を基準にしています。純正ファームやZMKが入った片側、純正無線ドングルとは組み合わせられません。メーカーの復元用ファームは含みません。

- [書き込み・左右登録・Bluetooth接続](docs/cornix-stock.md)
- [Studioで純正配列を試す](https://corex-studio.pages.dev/?preview=demo&device=cornix)
- [検証状況とピン定義の根拠](docs/cornix-stock-validation.md)

CoreX交換基板を使う場合は [CoreX-RMK](https://github.com/yuchamichami/CoreX-RMK) を使ってください。ここで配布するUF2は純正基板用です。

## 開発

[ビルド手順](BUILDING.md) / [変更履歴](CHANGELOG.md) / [第三者ライセンス](THIRD_PARTY_NOTICES.md)

CoreX-RMKから履歴と共通RMKソースを引き継ぎ、純正左右セットのビルド・配布・CIを独立させました。Studio本体は別管理です。機種のUIDと設定形式は分離前から変更していません。

CornixおよびVialの公式ファームウェア／公式アプリではありません。各ソースのライセンス宣言と第三者の著作権表示を維持しています。
