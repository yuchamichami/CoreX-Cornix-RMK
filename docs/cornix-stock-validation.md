# Cornix純正基板の対応根拠（2026-10-25）

この記録は純正基板向け試験版の開発資料です。メーカー純正ファームをそのままStudioで扱う対応ではありません。

## ピン定義

[hitsmaxft/zmk-keyboard-cornix](https://github.com/hitsmaxft/zmk-keyboard-cornix/tree/b2d5712fa3ef43ccbb796d3df1d48b78d59d27df/boards/jzf/cornix) の `cornix.dtsi`、`cornix_left_common.dtsi`、`cornix_right.dts`、`cornix_sensors.dtsi`、`nrf_e73.dtsi` を確認しました。[adong660/rmk-cornix](https://github.com/adong660/rmk-cornix) の行列・ダイヤル定義とも照合しています。GPIO番号の事実を取り出し、ドライバはCoreXの既存処理を使います。

| 用途 | 純正左 | 純正右 |
| --- | --- | --- |
| 行（順序保持） | P0.30, P0.31, P0.29, P0.02 | P1.09, P0.28, P0.03, P1.10 |
| 列（順序保持） | P0.28, P0.03, P1.10, P1.11, P1.13, P0.09, P0.10 | P0.30, P0.31, P0.29, P0.02, P1.13, P0.10, P0.09 |
| ダイヤル A/B | P1.04 / P1.06 | P1.04 / P1.06 |
| RGB電源 / データ | P0.13 / P0.24 | P0.24 / P0.13 |
| 電池ADC | P0.05 | P0.05 |

行列はcol2row、列HIGH・行プルダウン。ダイヤルは内部プルアップ、4エッジ／1操作。電池分圧は2000 / 2806。LEDは2個のGRB WS2812です。ただし参照ZMKのLEDノードは無効化されており、ピン定義を確認できたことと、CoreXドライバが実機で動作したことは区別しています。

## 配列と書き込み領域

メーカー [jezailfunder/cornix-lp](https://github.com/jezailfunder/cornix-lp/tree/496cee8811e7ed48987595857aeb6adc5b30c7c4) の `cornix固件 V1.12` にある左右UF2を確認。両方ともnRF52840 family `0xADA52840`、開始アドレス `0x1000` でした。純正左UF2のSHA256は `d83910f2dd54972ec7de24960cfafafdbb91609ddf5992bfcb56b46222337c9f`。

`keymaps/reference/cornix-v1.12-definition.json` は、その左UF2に埋め込まれたVial配列データ（flash `0x64c5b`、xz 752 bytes）です。メーカーの実行バイナリはこのリポジトリに再配布しません。初期キー割り当ては既存の `keymaps/reference/cornix-default-keymap.vil` を使い、その出典は `keymaps/README.md` に記録しています。

右をcentral、左をperipheralとし、全10レイヤーの行0–3／4–7とダイヤル0／1を交換します。欠けるキーやポインター用の仮想セルは作りません。Vial UIDは `CoreXST1`、Studioの機種IDは `cornix-stock`。右アプリは `0x1000–0xA0000` 未満に限定し、設定は `0xC0000–0xE0000`。UF2の変換時にELFのロード位置、ベクタ、上限を検証します。ブートローダは配布UF2に含みません。

## 再現

```sh
python3 tools/stock_cornix.py --check
./build.sh right
./build.sh left
python3 tools/package_stock.py
python3 tools/test_stock_cornix.py
./tools/test.sh
```

`./build.sh all` は純正左右の2種類をビルドし、`package_stock.py` が検証して `firmware/` に配置します。リポジトリ分離時の配布UF2は左右とも分離前のファイルをそのまま引き継いでいます。再ビルド結果はビルド環境によりハッシュが変わる場合があります。Studioはこのフォルダのmanifestに含むUID・版・圧縮定義サイズ・SHA256と10レイヤーを照合してから編集を許可します。

この作業では実機への書き込みを行っていません。実機未確認項目は[利用案内](cornix-stock.md#検証状況)に記載しています。
