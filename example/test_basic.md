---
title: Hello!
wew: true
---

``` {settings=""}
macros:
  wew: {{ json_to_str site.test2 }} 
```

# Hello, world!

This is include: {{> test_include.md }}

This is a test from TIMSync!

```
%%wew%%
```

wew: {{ wew }}

site.test: {{ site.test }}

This is another test text!

This is a link: [Testi](test_other_file)

This is a template value: {{ 2 }}

{{area "test"}}

This is an area!

**wew**

{{/area}}

## Hello, more text!

Today, we are going to program y...

Table 1: This is a table

| This | is    | a   | table |
|------|-------|-----|-------|
| This | works | too | wow!  |

### Level 3

This is a third level, wow!