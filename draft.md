# linear-tui 実装計画

Linear.app の TUI クライアントを Rust で構築する。

## 技術スタック

| カテゴリ             | 選定                                       | 理由                                                    |
| -------------------- | ------------------------------------------ | ------------------------------------------------------- |
| TUI フレームワーク   | [ratatui](https://ratatui.rs/) + crossterm | Rust TUI のデファクト。活発なメンテナンス               |
| 非同期ランタイム     | tokio                                      | GraphQL クライアントとの相性                            |
| GraphQL クライアント | graphql_client + reqwest                   | 型安全なクエリ生成。Linear SDK は TS のみのため自前構築 |
| 設定管理             | directories + toml / serde                 | XDG 準拠の config 配置                                  |
| キーマップ           | crossterm イベント処理                     | Vim ライクなキーバインド                                |

## Linear API 概要

- エンドポイント: `https://api.linear.app/graphql`
- 認証: OAuth2 + PKCE (推奨) / Personal API Key (フォールバック)
- ページネーション: Relay-style cursor-based
- レート制限: あり（公平なアクセスのため）

### OAuth2 認証フロー

1. ローカルに一時 HTTP サーバー起動 (`http://localhost:<port>/callback`)
2. ブラウザで `https://linear.app/oauth/authorize` を開く (PKCE `code_challenge` 付き)
3. ユーザーが承認 → コールバックで認可コード受領
4. `https://api.linear.app/oauth/token` でアクセストークン + リフレッシュトークン取得
5. トークンをローカルに安全に保存 (keyring or encrypted file)
6. アクセストークン期限切れ (24h) 時はリフレッシュトークンで自動更新

**スコープ**: `read`, `write` (必要に応じて `issues:create`, `comments:create` 等で最小権限化)

**初回セットアップ**: `linear-tui auth login` コマンドで OAuth フローを開始。API Key を使う場合は `linear-tui auth token <key>` で設定。

## アーキテクチャ

```
src/
├── main.rs              # エントリポイント、tokio ランタイム起動
├── app.rs               # App 状態管理 (Model)
├── event.rs             # イベントハンドラ (Controller)
├── ui/                  # 描画ロジック (View)
│   ├── mod.rs
│   ├── issue_list.rs    # Issue 一覧画面
│   ├── issue_detail.rs  # Issue 詳細画面
│   ├── project_list.rs  # Project 一覧
│   ├── cycle_view.rs    # Cycle ビュー
│   └── help.rs          # ヘルプ画面
├── api/                 # Linear API クライアント
│   ├── mod.rs
│   ├── client.rs        # GraphQL クライアント
│   ├── queries/         # .graphql ファイル群
│   │   ├── issues.graphql
│   │   ├── projects.graphql
│   │   ├── teams.graphql
│   │   └── cycles.graphql
│   └── types.rs         # API レスポンス型
├── auth/                # 認証モジュール
│   ├── mod.rs
│   ├── oauth.rs         # OAuth2 + PKCE フロー
│   ├── token.rs         # トークン保存・更新
│   └── server.rs        # コールバック用ローカル HTTP サーバー
├── config.rs            # 設定ファイル読み込み
└── keys.rs              # キーバインド定義
```

MVC パターンを採用。ratatui は即時モードレンダリングのため、メインループで「イベント取得 → 状態更新 → 描画」を繰り返す。

## 画面構成

### 1. Issue 一覧 (メイン画面)

```
┌─ Team: Core ──────────────────────────────────────┐
│ Filter: [All] [My Issues] [Active] [Backlog]      │
├───────┬───────────────────────┬──────────┬────────┤
│ ID    │ Title                 │ Status   │ Pri    │
├───────┼───────────────────────┼──────────┼────────┤
│ COR-1 │ Fix login bug         │ In Prog  │ Urgent │
│ COR-2 │ Add dark mode         │ Todo     │ High   │
│ COR-3 │ Update docs           │ Done     │ Low    │
│ ...   │                       │          │        │
├───────┴───────────────────────┴──────────┴────────┤
│ j/k: move  Enter: detail  /: search  ?: help      │
└───────────────────────────────────────────────────┘
```

### 2. Issue 詳細

```
┌─ COR-1: Fix login bug ───────────────────────────┐
│ Status: In Progress    Priority: Urgent            │
│ Assignee: @user        Labels: [bug] [auth]        │
│ Project: Q1 Sprint     Cycle: Sprint 12            │
├───────────────────────────────────────────────────┤
│ Description:                                       │
│ Login fails when using SSO with...                 │
│                                                    │
├─ Comments ────────────────────────────────────────┤
│ @alice (2h ago): Reproduced on staging             │
│ @bob (1h ago): PR is up for review                 │
├───────────────────────────────────────────────────┤
│ Esc: back  s: status  a: assign  c: comment       │
└───────────────────────────────────────────────────┘
```

### 3. Project 一覧 / Cycle ビュー

Issue 一覧と同様のテーブル形式。プロジェクト配下の Issue をグルーピング表示。

## キーバインド (Vim ライク)

| キー        | アクション                                  |
| ----------- | ------------------------------------------- |
| `j` / `k`   | カーソル上下                                |
| `Enter`     | 詳細表示                                    |
| `Esc` / `q` | 戻る / 終了                                 |
| `/`         | 検索                                        |
| `f`         | フィルタ切替                                |
| `t`         | Team 切替                                   |
| `s`         | ステータス変更                              |
| `a`         | アサイン変更                                |
| `c`         | コメント追加                                |
| `p`         | Priority 変更                               |
| `r`         | リロード                                    |
| `?`         | ヘルプ                                      |
| `1-4`       | タブ切替 (Issues/Projects/Cycles/My Issues) |

## 実装フェーズ

### Phase 1: 基盤構築

- [ ] `cargo init` でプロジェクト初期化
- [ ] 依存クレートの追加
- [ ] 設定ファイル (`~/.config/linear-tui/config.toml`) の読み込み
- [ ] OAuth2 + PKCE 認証フロー (`linear-tui auth login`)
  - ローカル HTTP サーバーでコールバック受信
  - トークン保存・リフレッシュ機構
  - Personal API Key フォールバック対応
- [ ] Linear API クライアントの基本実装
  - 認証ヘッダ付き GraphQL リクエスト (OAuth / API Key 両対応)
  - エラーハンドリング
- [ ] メインループの骨格 (イベント処理 + 描画)

### Phase 2: Issue 一覧 & 詳細

- [ ] Team 一覧取得・選択
- [ ] Issue 一覧表示 (テーブル)
  - ページネーション (cursor-based スクロール)
  - ステータス・Priority のカラー表示
- [ ] Issue 詳細表示
  - Description のマークダウン簡易レンダリング
  - コメント一覧
- [ ] フィルタ機能 (ステータス、アサイン、Priority)
- [ ] 検索機能

### Phase 3: ミューテーション

- [ ] Issue ステータス変更
- [ ] Issue アサイン変更
- [ ] Issue Priority 変更
- [ ] コメント投稿

### Phase 4: 追加画面

- [ ] Project 一覧・詳細
- [ ] Cycle ビュー
- [ ] My Issues ビュー (自分にアサインされた Issue)

### Phase 5: UX 改善

- [ ] ローディングインジケータ
- [ ] エラー表示 (ポップアップ)
- [ ] キャッシュ (前回取得データの保持)
- [ ] テーマ設定 (カラーカスタマイズ)

## 設定ファイル例

```toml
# ~/.config/linear-tui/config.toml

[auth]
# OAuth トークンは `linear-tui auth login` で自動設定される
# Personal API Key を使う場合は以下を設定
# api_key = "lin_api_xxxxx"

[ui]
default_team = "Core"
theme = "default"
items_per_page = 50

[keybindings]
# カスタムキーバインドがあればここに
```

## 主要な依存クレート

```toml
[dependencies]
ratatui = "0.29"
crossterm = "0.28"
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json"] }
graphql_client = "0.14"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
directories = "5"
anyhow = "1"
```
