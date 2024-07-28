---
title: Custom helpers test
---

# Custom helpers test

TIMSync allows basic scripting via custom helpers.
These helpers are defined in the `_helpers` folder in the project root.
Helpers use the [rhai](https://rhai.rs/book/about/index.html) scripting language.

## Example

The following string is generated using a custom helper: {{hello "TIM world"}}.