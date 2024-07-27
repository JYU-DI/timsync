---
title: Task import test
---

# Task import test

## Importing using the `task` function

Using the `task` function to import a task from a document.

The only available parameters:

- `uid` is the unique ID of the task to import. This value is unique to the entire project.
- The task is defined in this project using the identifier above.
- The function automatically handles creating the relevant references and generating a task.

# Examples

## Example 1: Basic task

{{task "task1"}}

## Example 2: Task with imported contents

{{task "task2"}}

