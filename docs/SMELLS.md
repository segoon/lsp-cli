# map_or_else()

Instead of:

```rust
expr().map_or_else(
    || const_expr(),
    func,
);
```

use this:

```rust
if let Some(tmp) = expr() {
    tmp.func()
} else {
    const_expr()
}
```

# ok_or_else()

Instead of:

```rust
let response = expr().ok_or_else(|| {
    error_expr()
})?;
```

use this:

```rust
let Some(response) = expr() else {
    return Err(error_expr());
};
```


# then()

Instead of:

```rust
expr().then(|| then_expr())
```

use this:

```rust
if expr() {
    Some(then_expr())
} else {
    None
}
```
