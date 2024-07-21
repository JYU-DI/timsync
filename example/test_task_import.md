---
title: Task import test
---

# Task import test

## Importing via area reference

Using the `ref` function to import a task from a document.

Needed parameters:

- `doc` is the document to import from. For now, it can only be an ID of an existing document in the TIM instance where
  the document is being imported.
- `area` is the area to import from. It is assumed that the task is defined in the area.

> TODO: The `doc` must be a docId for now. In the future, it can be a relative path in which case the ID would be
> automatically resolved.

{{{{raw}}}}
{{ref-area "test_basic" area="test"}}
{{{{/raw}}}}

## Importing using the `task` function

> **This is not yet implemented and is meant as a spec of a future feature.**

Using the `task` function to import a task from a document.

The only available parameters:

- `uid` is the unique ID of the task to import. This value is unique to the entire project.
- The task is defined in this project using the identifier above.
- The function automatically handles creating the relevant references and generating a task.

{{{{raw}}}}

{{task "task1"}}

{{{{/raw}}}}