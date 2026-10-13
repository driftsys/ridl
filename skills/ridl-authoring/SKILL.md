---
name: ridl-authoring
description: >-
  Write, review and evolve RIDL-family contracts: typl types, ridl interfaces
  and rsdl systems in a workspace with a ridl.toml. Use when creating or
  editing .typl, .ridl or .rsdl files or a ridl.toml, when turning a
  requirement into an interface or a type, or when reviewing such a change.
metadata:
  version: "0.10.0"
---

# Authoring RIDL

## The model in one minute

```typl
package skill.intro

/// Cabin air temperature, in degrees Celsius.
type Temperature : Cel [-40.0..85.0 step 0.5]
```

## The verify loop

## Which reference to read
