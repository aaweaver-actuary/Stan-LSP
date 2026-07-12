# Fixture conventions

Language fixtures use UTF-8 byte offsets in normalized `.expected.json` sidecars.
Protocol tests express the same expectations as UTF-16 line/character ranges.
Formatting fixtures use `.expected.stan` because exact output is the contract.
All fixture readers preserve source order and must not depend on hash-map ordering.
