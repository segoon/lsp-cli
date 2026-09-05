# Existing playgrounds are not all valid standalone projects

`E2E_TESTS.md` requires every source-language playground to be valid, but the existing C++ fixture
does not link because `main.cpp` calls undefined `f()` and `g()`. The Rust fixture also cannot be
checked through its own manifest because Cargo treats the nested package as an undeclared member
of the repository's root package workspace.

The project audit must distinguish source presence from validated buildability. Repair these
fixtures in the dedicated cleanup step rather than silently treating their files as a valid E2E
baseline.
