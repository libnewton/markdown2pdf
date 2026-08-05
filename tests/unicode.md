---
title: Unicode — 多语言排版
lang: zh
authors:
  - 张三
  - Ada Lovelace
---

# 标题一 — a heading in two scripts

This fixture exists because the engine is UTF-8 end to end but the *fonts*
were not: `fonts/` carries Latin only, so Chinese used to render as tofu in
the PDF while the HTML looked fine. It also pins the parts of the pipeline
that index by byte — slugs, emoji matching, table alignment, highlighting.

## 中文

简体中文段落。**粗体**、_斜体_、`行内代码`、~~删除线~~，以及一个[链接](https://example.com)。

标点符号：，。、；：？！「」『』（）《》——…… and mixed 中英文 in one line.

## 日本語

ひらがな、カタカナ、漢字が混ざった文章です。**太字**と_斜体_も動きます。

## 한국어

한글 문장입니다. 자모 조합과 **굵게** 표시가 모두 동작해야 합니다.

## Right to left

العربية: مرحبا بالعالم. Hebrew: שלום עולם. Mixed into English mid-sentence.

## Combining marks and normalisation

Composed é vs decomposed é, Å vs Å, and a stack: q̈̊ẍ̆.

## A table across scripts

| Language | Sample        | Note        |
| -------- | ------------- | ----------- |
| 中文     | 你好，世界    | Simplified  |
| 日本語   | こんにちは    | Kana + kanji |
| 한국어   | 안녕하세요    | Hangul      |
| العربية  | مرحبا         | RTL         |

## Headings that have to slug

### 你好世界

### Ünï cödé

### 混合 mixed 标题

## Code with wide characters

```python
# 注释：宽字符不应破坏语法高亮
名前 = "こんにちは"
def 挨拶(who: str) -> str:
    return f"{名前}, {who}!"
```

## Emoji next to CJK

工作 ✅ 完了 🎉 and a flag 🇯🇵 and a ZWJ family 👨‍👩‍👧.

- [ ] 未完成的任务
- [x] 完成的任务
