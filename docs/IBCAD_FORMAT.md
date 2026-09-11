# `.ibcad` 保存形式

## 目的

`.ibcad`は、ハコニワのProjectを単一ファイルで保存するZIPコンテナである。作品の論理状態を完全に復元し、読込後にApplication Commandで編集を継続できることを保証する。

## コンテナ構成

| パス | 必須 | 内容 |
| --- | --- | --- |
| `project.json` | 必須 | 形式バージョンとProjectの論理状態 |
| `palette.json` | 任意 | 将来のパレットスナップショット |
| `thumbnail.png` | 任意 | 将来のサムネイル |

現在の`format_version`は`2`である。

## v2の論理スキーマ

`project.json`のトップレベルは次の2フィールドを持つ。

| フィールド | 型 | 内容 |
| --- | --- | --- |
| `format_version` | 整数 | 現在は`2` |
| `project` | オブジェクト | 復元対象のDomain Project |

`project`には以下を保存する。

- Project名と次に発行する安定ID
- AssemblyのID・名称・Root Group ID
- GroupのID・名称・親Group・同階層での表示順・配置・表示状態
- Shape/PieceのID・名称・親Group・同階層での表示順・配置・表示状態
- 全Beadの格子座標とRGB色
- Pieceのローカル平面
- PDF用紙と組立ガイド有無を含む出力設定

格子座標は`"x,y,z"`形式の文字列として表現する。色は`[r,g,b]`、平面は`{"Xy":{"z":0}}`、`{"Xz":{"y":0}}`、`{"Yz":{"x":0}}`のいずれかである。姿勢は`"Xy"`、`"Xz"`、`"Yz"`のいずれかであり、任意角度は保存できない。

すべてのコレクションは安定IDまたは座標順の決定的な順序で保存する。同じProjectを複数回保存した場合、`project.json`だけでなくZIP全体が同じバイト列になるよう、エントリ順、圧縮方式、タイムスタンプ、権限値を固定する。

## 空Projectの例

```json
{
  "format_version": 2,
  "project": {
    "name": "empty",
    "assembly": {
      "id": 1,
      "name": "Assembly",
      "root_group_id": 2
    },
    "groups": {
      "2": {
        "id": 2,
        "name": "Root",
        "parent_group_id": null,
        "sibling_order": 0,
        "placement": {
          "translation": "0,0,0",
          "orientation": "Xy"
        },
        "visible": true
      }
    },
    "shapes": {},
    "pieces": {},
    "output_settings": {
      "page_size": "A4",
      "include_assembly_guide": true
    },
    "next_id": 3
  }
}
```

## ハンマー形状の最小例

次の例は、頭部と柄がそれぞれPiece化済みで、Assemblyが出力可能な最小構成を示す。

```json
{
  "format_version": 2,
  "project": {
    "name": "hammer",
    "assembly": {
      "id": 1,
      "name": "Assembly",
      "root_group_id": 2
    },
    "groups": {
      "2": {
        "id": 2,
        "name": "Root",
        "parent_group_id": null,
        "sibling_order": 0,
        "placement": {
          "translation": "0,0,0",
          "orientation": "Xy"
        },
        "visible": true
      }
    },
    "shapes": {},
    "pieces": {
      "3": {
        "id": 3,
        "name": "head",
        "parent_group_id": 2,
        "sibling_order": 0,
        "placement": {
          "translation": "0,0,0",
          "orientation": "Xy"
        },
        "visible": true,
        "plane": { "Xy": { "z": 0 } },
        "beads": {
          "0,0,0": [220, 58, 52]
        }
      },
      "4": {
        "id": 4,
        "name": "handle",
        "parent_group_id": 2,
        "sibling_order": 1,
        "placement": {
          "translation": "0,0,0",
          "orientation": "Xz"
        },
        "visible": true,
        "plane": { "Xz": { "y": 0 } },
        "beads": {
          "0,0,0": [117, 78, 47]
        }
      }
    },
    "output_settings": {
      "page_size": "A4",
      "include_assembly_guide": true
    },
    "next_id": 5
  }
}
```

## 保存しない状態

以下はPresentationまたはRendererの一時状態であり、作品ファイルには含めない。

- 選択ハイライト、選択中タブ、カメラ位置
- ドッキングレイアウト、ペインサイズ、開閉状態
- 描画メッシュ、GPUリソース、Dirty region
- 構造警告の表示キャッシュ
- Undo/Redo履歴

## 読込・検証規則

- ZIPでないファイルは読込エラーとする。
- `project.json`欠損は専用エラーとする。
- 不正JSONはJSON読込エラーとする。
- 未知の`format_version`は推測せず拒否する。
- 読込後にIDの一意性、MapキーとEntity IDの一致、Root Group、親子関係、循環、Piece平面制約、次IDをDomainで再検証する。
- 保存前にも同じDomain検証を行い、不正Projectで既存ファイルを作成・上書きしない。

## v1からの移行

v1はProject名、Shape、Piece、Bead、Piece平面、次IDのみを持つ。読込時に次の明示変換を行う。

1. 既存IDより大きいIDでAssemblyとRoot Groupを生成する。
2. すべてのShape/PieceをRoot Group直下へ配置する。
3. 配置を原点、表示状態を表示へ設定する。
4. Piece姿勢を保存済み平面から導出する。
5. 出力設定をA4・組立ガイド有効へ設定する。
6. 移行後のProjectをDomainで再検証する。

v1ファイルは読み込めるが、次回保存時は必ずv2として保存する。v0およびv3以降は、対応コードが追加されるまで拒否する。
