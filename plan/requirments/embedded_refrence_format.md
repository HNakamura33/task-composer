以下のような仕様にしたい。

1. 依存ノードの出力を埋め込める

```json
{
...
  "args": {
     "prompt": "..., previous output: $.1.output.content.result"
  }
}
```

2. 自身のノードの別のフィールドの値を参照および埋め込みできる
```json
{
   "prompt": "..."
   "args"{
     "prompt": $.self.prompt
   }
   
}
```
