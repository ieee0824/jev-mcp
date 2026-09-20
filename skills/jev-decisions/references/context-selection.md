# コンテキスト保持候補の選別

長い作業履歴から、次の判断に必要なツール結果や観測を残す・短くする・作業コンテキストから外す候補を選ぶときに使う。このレシピは元の会話、ログ、ファイルを削除しない。

## 先に固定する情報

呼び出し側は、次の情報をJevの選別対象にせず必ず保持する。

- ユーザーが示した目的、制約、許可、未回答の要求。
- 採用した設計と、その理由や重要なトレードオフ。
- 不具合の再現手順と、結果を比較する条件。
- 現在の未解決事項、既知のリスク、残っている検証。

これらは `pinned_context` として短く整理できるが、保持するかどうかをJevに決めさせない。

## 候補の作り方

残りの履歴を、出所と意味が分かる小さな候補へ分ける。現在成功している結果だけでなく、再発条件、棄却した仮説、以前の失敗を示す観測も候補に含める。現在の成功と矛盾する過去の失敗は、環境差や不安定性を示す可能性がある。

各候補について、次の行動または未解決の判断に必要かを Noul で尋ね、1回の Batchにまとめる。候補の重要度を段階で比較したい場合は、同じ基準を持つ Score を候補ごとに使える。複数候補が同時に必要になり得るため、通常はChoiceを使わない。

## Batch の例

```json
{
  "state": {
    "next_decision": "再試行修正を提出できるか判断する。",
    "pinned_context": {
      "user_constraint": "公開APIの形式を変えない。",
      "design_reason": "再試行上限の既定値を保ち、明示設定だけを尊重する。",
      "reproduction": "一時エラー時に期待4回、実測1回だった。",
      "unresolved": "全テストと静的検査が未実施。"
    },
    "candidates": {
      "focused_success": "修正後、対象の再試行テストは成功した。",
      "past_failure": "修正前は同じテストが期待4回、実測1回で失敗した。",
      "unrelated_format": "ドキュメント整形テストが改行差分で失敗したが、再実行では成功した。",
      "discarded_api_hypothesis": "APIは202を返し、ジョブ登録要求を受け付けていたため、API原因説を棄却した。"
    }
  },
  "questions": {
    "keep_focused_success": {
      "type": "noul",
      "instructions": "`candidates.focused_success` は、`next_decision` で修正後の対象動作を確認するために保持すべき証拠ですか？"
    },
    "keep_past_failure": {
      "type": "noul",
      "instructions": "`candidates.past_failure` は、再現条件と修正前後の差を保つために `next_decision` の作業コンテキストへ保持すべき証拠ですか？"
    },
    "keep_unrelated_format": {
      "type": "noul",
      "instructions": "`candidates.unrelated_format` は、再試行修正の提出可否という `next_decision` に必要なため作業コンテキストへ保持すべき証拠ですか？"
    },
    "keep_discarded_api_hypothesis": {
      "type": "noul",
      "instructions": "`candidates.discarded_api_hypothesis` は、同じ誤った調査を繰り返さず原因説明を保つために `next_decision` の作業コンテキストへ保持すべき証拠ですか？"
    }
  }
}
```

## 結果の反映

- 強く必要と評価された候補は、出所と観測を保って残す。
- 不確かな候補は、意味を失わない短い要約にできるか呼び出し側が判断する。
- 必要性が低い候補は、現在の作業コンテキストから外せる。ただし元の会話、ログ、ファイルを削除する許可にはならない。
- `pinned_context` は結果にかかわらず保持する。

選別後も、次の判断に必要な反証や過去の失敗が残っているかをCodexが確認する。Jevの評価だけを理由に、監査記録やユーザーデータの保存方針を変えない。
