# `.ibcad` 保存形式

## 目的

`.ibcad`は、ハコニワのProjectを単一ファイルで保存するためのZIPコンテナである。現在のMVPでは、作品の論理状態を完全に復元できることを優先する。

## コンテナ構成

| パス | 必須 | 内容 |
| --- | --- | --- |
| `project.json` | 必須 | Project、Shape、Piece、Bead、形式バージョン |
| `palette.json` | 任意 | 将来のパレットスナップショット |
| `thumbnail.png` | 任意 | 将来のサムネイル |

## `project.json`

MVPのJSONには、トップレベルで`format_version`と`project`を持たせる。現在の`format_version`は`1`とする。

保存する論理状態:

- Project名、次に発行するID
- Shape/PieceのID、名称、全Beadの格子座標と色
- Pieceの平面（XY/XZ/YZ）

保存しない状態:

- 選択状態、カメラ位置、ドッキングレイアウト
- 描画メッシュ、GPUリソース、構造警告の描画キャッシュ
- Undo/Redo履歴

## 読込規則

- ZIPでない、または`project.json`がないファイルは読込エラーとする。
- 未知の`format_version`は読込を拒否する。
- 読込後にすべてのPieceを再検証する。平面制約違反があれば読込を拒否する。
- 形式の後方互換性が必要になった場合は、旧バージョンから現在のDomainモデルへの明示的な変換を追加する。暗黙の推測はしない。

## MVPサンプル

ハンマーは、頭部と柄を別Pieceとして保存する。出力対象にする前に、ShapeのBeadをすべてPieceへ変換済みであることを確認する。
