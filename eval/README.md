# 日本語コーディング判断の最小評価セット

`ja-coding-minimal.jsonl` は、Jev を日本語のコーディング判断に使う際の小さな回帰確認用 fixture です。すべて架空のデータで、テスト失敗との関連性、調査先、変更リスクを Noul・Choice・Score 各4件で確認します。明確な例に加えて、情報不足、複数候補、`unknown` を期待する例を含みます。

```sh
cargo run --locked -- eval validate eval/ja-coding-minimal.jsonl
```

実モデルで測る場合は、API利用料金が発生し得ることを確認してから実行します。

```sh
TYPESAFE_API_KEY="your_typesafe_api_key" \
  cargo run --locked -- eval run eval/ja-coding-minimal.jsonl \
  > eval/baselines/ja-coding-YYYY-MM-DD.jsonl
```

この12件はモデルや指示の変更による大きな回帰を見つけ、失敗例をレビューするための出発点です。少数の架空ケースの結果を、一般的な日本語性能や本番品質の保証として扱わないでください。

## ベースラインの記録

実測結果は fixture を変更せず、`eval/baselines/` の別ファイルに保存します。結果JSONLと一緒に、同じ名前の `.metadata.json` に次を記録します。

```json
{
  "run_date": "YYYY-MM-DD",
  "requested_model": "jev-latest",
  "resolved_models": ["実行結果に記録されたモデル版"],
  "summary": {
    "total": 12,
    "passed": 0,
    "failed": 0,
    "errors": 0,
    "input_tokens": 0,
    "output_tokens": 0,
    "average_elapsed_ms": 0
  }
}
```

`summary` は評価ランナー最終行の値を転記します。上のJSONは形式例であり、未実施の測定結果ではありません。
