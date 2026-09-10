# Hakoniwa 開発ガイド

## アーキテクチャ

Hakoniwaは、作品データの整合性をUI・描画・ファイル形式から独立させる、Rustのレイヤード・アーキテクチャを採用する。

```text
hakoniwa-app
  ├─ hakoniwa-presentation
  │    egui/eframe UI · wgpu描画 · カメラ · 選択 · ショートカット
  ├─ hakoniwa-application
  │    Command実行 · Undo/Redo · ユースケース
  ├─ hakoniwa-domain
  │    Project · Assembly · Shape · Piece · Bead
  │    階層 · 接合推定 · 構造チェック · 集計 · 組立順
  └─ hakoniwa-infrastructure
       .ibcad保存/読込 · PDF出力 · パレット · 設定
```

想定するCargo workspaceのクレート構成は次のとおり。

```text
crates/
  hakoniwa-domain/
  hakoniwa-application/
  hakoniwa-infrastructure/
  hakoniwa-presentation/
  hakoniwa-app/
```

### 依存と責務のルール

- `hakoniwa-domain` は作品データの唯一の正とする。egui、wgpu、PDF、ZIP、ファイルI/O、OS APIへ依存させない。
- `Project`、`Assembly`、`Shape`、`Piece`、`Bead`、階層、接合推定、構造チェック、集計、組立順はDomainに置く。
- `Piece`の「すべてのBeadが単一ローカル平面上にある」という不変条件はDomainで必ず検証する。
- PDF出力可否（未分割の`Shape`が残る場合は拒否する）はDomainのルールとして判定する。
- `hakoniwa-application` はDomainを変更する唯一の入口である。編集操作はCommandとして実装し、Undo/Redoできるようにする。
- `hakoniwa-presentation` はDomainを直接変更しない。UIイベントをApplication Commandへ変換する。
- 3D描画はDomain状態から導出するキャッシュとして扱う。DomainのPiece/Beadとwgpu用メッシュ・インスタンスの間にはAdapterを置く。
- `hakoniwa-infrastructure` はRepositoryやStrategyの実装を担う。`.ibcad`はZIPコンテナとし、少なくとも`project.json`と形式バージョンを持たせる。
- 依存方向はPresentation/Infrastructure → Application → Domainを基本とし、Domainから外側のレイヤーへ依存させない。

## Git運用

- コミットは小さく、レビュー・巻き戻し・切り分けが可能な単位で作る。
- コミットごとに、目的が分かる命令形のメッセージを付ける。複数の無関係な変更を1コミットへ混ぜない。
- 実装途中でも、意味のある完結した節目（例: Domainの値オブジェクト追加、1つのCommand追加、保存形式追加、UIの1機能追加）でコミットする。
- コミット前に、変更内容に応じたフォーマット・静的解析・テストを実行する。Rustコードでは原則として以下を通す。
  - `cargo fmt --check`
  - `cargo clippy --workspace -- -D warnings`
  - `cargo test --workspace`
- コミット後は、他者と共有すべき節目および作業終了時に`origin`へpushする。ローカルに未pushの意味あるコミットを長時間ため込まない。
- push前に、対象ブランチ、差分、リモートURLを確認する。履歴の書き換えやforce pushは、明示的な指示なしに行わない。

## 初期リリースの境界

- Pieceの姿勢はXY/XZ/YZの直交3平面に限定する。任意角度は扱わない。
- Shapeの自動分割・最適分割は将来機能。初期版は手動選択、選択面、XY/XZ/YZ平面によるPiece化支援までとする。
- 構造チェックは製作上の注意喚起であり、安全性を保証するものではない。
