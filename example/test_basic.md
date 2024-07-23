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

This is docid of another document: {{ site.doc.hello2.doc_id }}  
This is path of another document: {{ site.doc.hello2.path }}

Base path: {{ site.base_path }}

```
%%wew%%
```

wew: {{ wew }}

site.test: {{ site.test }}

This is another test text!

This is a link: [Testi](test_other_file)

This is a template value: {{ 2 }}

{{#area "test"}}
This is an area!
**wew**
{{/area}}

{{#area collapse=true}}
**Collapsible area!**
{{else}}
Area content!
{{/area}}

This is an area reference:

{{ref_area doc_id "test"}}

## Hello, more text!

Today, we are going to program y...

Table 1: This is a table

| This | is    | a   | table |
|------|-------|-----|-------|
| This | works | too | wow!  |

### Level 3

This is a third level, wow!