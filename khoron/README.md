# khoron

A tiny Lua-flavored scripting experiment for Cropbot.

It features dynamic typing, closures, classes, and modern shorthand operators.

### Features

*   **Shorthands**: Supports `++`, `--`, `+=`, `-=`, `*=`, `/=`.
*   **Comments**: Use `//` for single-line comments.
*   **Data Structures**: Built-in support for Lists `[1, 2, 3]` and Dictionaries `{"key": value}`.
*   **OOP**: Simple class and instance support.

### Example

```lua
// Functions and Lists
fn get_data() {
    return [10, 20, 30]
}

// Classes and Objects
class Bot {
    fn init(name) {
        self.name = name
        self.stats = {"level": 1}
    }
    fn greet() {
        print("Hello, I am", self.name)
        self.stats["level"] += 1 // Shorthand supported!
    }
}

my_bot = Bot("CropBot")
my_bot.greet()
print("Level is now", my_bot.stats["level"])
```

### Running

Run a file:
```sh
cargo run -p khoron -- script.cbl
```

Or pipe source through stdin:
```sh
echo 'print("hello " + "world")' | cargo run -p khoron
```
