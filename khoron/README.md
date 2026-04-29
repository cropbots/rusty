# khoron

A tiny Lua-flavored scripting experiment for Cropbot.

It intentionally keeps one global scope and avoids language keywords. Supported statements are
assignments and expression calls:

```lua
name = "crop" + "bot"
score = 2 + 3 * 4
print(name + " scored " + score)
```

Unlike Lua, string concatenation uses `+` instead of `..`. When both operands are numbers, `+`
performs numeric addition. If either operand is a string, both sides are converted to text and
concatenated.

Run a file:

```sh
cargo run -p khoron -- script.cbl
```

Or pipe source through stdin:

```sh
echo 'print("hello " + "bot")' | cargo run -p khoron
```
