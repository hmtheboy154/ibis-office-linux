# Third-Party Notices

[English](#third-party-notices) | [日本語](#サードパーティの表示日本語)

ibisOffice includes third-party components (Rust crates, fonts, icons, and data files from the ECMA-376 standard). Their copyrights belong to their respective authors, and each component is used under its own license. Those licenses are not replaced by the AGPL-3.0 or the commercial license of ibisOffice. The commercial license covers only the parts in which ibis inc. holds the copyright.

The complete list of components, with their licenses and the notices those licenses require, is generated from the dependencies of each build:

- Web version: [www/license.md](www/license.md)
- Desktop version (macOS): [crates/ibis-office-app/macos/license.md](crates/ibis-office-app/macos/license.md) (also included in `ibisOffice.app` under `Contents/Resources/Licenses/`)
- Desktop version (Linux): [crates/ibis-office-app/linux/license.md](crates/ibis-office-app/linux/license.md) (also installed under `$PREFIX/share/doc/ibis-office/`)

Only components under licenses that allow commercial use are included (Apache-2.0, MIT, BSD, MPL-2.0, ISC, Zlib, Unicode, CC0, BSL-1.0, 0BSD, OFL-1.1, and the ECMA-376 data under the Ecma copyright policy). Copyleft licenses such as GPL, LGPL and AGPL are not used for third-party components.

## サードパーティの表示（日本語）

ibisOffice には、第三者の部品（Rust の crate、フォント、アイコン、ECMA-376 規格のデータファイル）が含まれています。これらの著作権はそれぞれの作者に帰属し、各部品はそれぞれのライセンスで利用しています。これらのライセンスは、ibisOffice の AGPL-3.0 や商用ライセンスで置き換わるものではありません。商用ライセンスの対象は、株式会社アイビスが著作権を持つ部分だけです。

部品の完全な一覧と、そのライセンスが求める表示は、各ビルドの依存から生成しています。

- Web 版：[www/license.md](www/license.md)
- デスクトップ版（macOS）：[crates/ibis-office-app/macos/license.md](crates/ibis-office-app/macos/license.md)（`ibisOffice.app` の `Contents/Resources/Licenses/` にも入っています）
- デスクトップ版（Linux）：[crates/ibis-office-app/linux/license.md](crates/ibis-office-app/linux/license.md)（`$PREFIX/share/doc/ibis-office/` にも入っています）

商用で使えるライセンス（Apache-2.0、MIT、BSD、MPL-2.0、ISC、Zlib、Unicode、CC0、BSL-1.0、0BSD、OFL-1.1、および Ecma テキスト著作権ポリシーの ECMA-376 データ）の部品だけを使っています。GPL・LGPL・AGPL などのコピーレフトのライセンスは、第三者の部品には使っていません。
