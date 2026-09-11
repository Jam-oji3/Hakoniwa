# アーキテクチャ

## クレート境界

| クレート | 責務 | 許可する依存 |
| --- | --- | --- |
| `hakoniwa-domain` | 作品データ、検証、接合・構造規則 | Rust標準ライブラリのみを基本とする |
| `hakoniwa-application` | Command、Undo/Redo、ユースケース | `hakoniwa-domain` |
| `hakoniwa-infrastructure` | `.ibcad`、PDF、パレット、設定 | `hakoniwa-domain`、`hakoniwa-application`の公開抽象 |
| `hakoniwa-presentation` | egui UI、wgpu描画、入力・選択 | `hakoniwa-application`、`hakoniwa-domain`の読取API |
| `hakoniwa-app` | 実行ファイル、依存の組み立て | 上記すべて |

## 依存方向

```text
presentation ─┐
              ├─ application ── domain
infrastructure┘

app ── presentation + infrastructure + application
```

`domain`は外側のレイヤーを参照しない。UIイベント、ファイル読込、PDF出力はいずれもApplicationのCommandまたは読み取りユースケースを通してDomainに到達する。

## 状態管理

- `Project`が作品に関する唯一の正である。
- 編集はCommandとして実行し、成功したCommandだけをUndo/Redo履歴へ積む。
- 描画メッシュ、選択ハイライト、カメラ位置などは派生または一時状態であり、Domainの作品状態ではない。
- 保存形式はM3着手時に`docs/IBCAD_FORMAT.md`で確定する。

## ワークスペースUI

- `hakoniwa-presentation`は`egui_tiles`でペイン配置を管理する。
- 初期レイアウトは「パーツツリー」「2Dエディタ」「3D View」「インスペクタ」の4列とし、各ペインは常にタブを持つ。
- タブはドラッグによる移動・タブ化・縦横分割、および×ボタンによる一時的な非表示に対応する。
- 閉じたペインは上部の「表示」メニューから再表示でき、初期レイアウトへのリセットも提供する。
- ペイン配置、サイズ、開閉状態はPresentation固有のUI状態とし、`Project`や`.ibcad`には含めない。
