# ビルドと検証

Rust 1.95.0、Python 3.11以降、`flip-link` 0.1.12を使います。別リポジトリのチェックアウトは不要です。

```sh
rustup toolchain install 1.95.0 --profile minimal
rustup target add thumbv7em-none-eabihf --toolchain 1.95.0
rustup component add llvm-tools-preview --toolchain 1.95.0
cargo install flip-link --version 0.1.12 --locked
./build.sh all
python3 tools/package_stock.py
./tools/test.sh
```

片側だけの場合は `./build.sh right` または `./build.sh left` を指定します。省略時は左右両方です。`s140v6`／`s140v7`の選択はありません。右・左とも純正基板専用の開始アドレスを使い、接続された本体へは書き込みません。

初回はCargo依存関係をダウンロードします。UbuntuではCコンパイラ、pkg-config、libclangとArm組み込み向けGCCを先に用意します。

```sh
sudo apt-get install build-essential pkg-config libclang-dev gcc-arm-none-eabi libnewlib-arm-none-eabi
```

出力は `build/firmware/` です。`package_stock.py` は左右のELF・UF2の一致、起動ベクタ、書き込み範囲、コンパイル済みVial定義を照合してから `firmware/` に配置します。`python3 tools/package_stock.py --check` は配布物を書き換えずに再ビルドと機種識別の一致を検証します。配布物だけの検証には `python3 tools/verify_stock.py` を使います。

機種UID `436f726558535431`、v0.11.4の識別値 `2820` と圧縮Vial定義は分離前と同じです。左右とも `0x1000` から起動します。設定領域とブートローダを通常のUF2へ混ぜません。[ピン定義と検証範囲](docs/cornix-stock-validation.md)

`tools/git-metadata/git` は保存形式の互換性に使うRMK基準commitを固定し、ビルド用パスは配布バイナリから除去します。分離元の共有ドライバ・テスト・設定資料はソースに保持していますが、ビルド入口は純正基板用だけです。基板設定とfeatureを取り違えないよう、ルートの `build.sh` を使ってください。
