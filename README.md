# tianshu-formula

确定性公式解析与求值：同样的表达式和变量，永远得到同一个 `f64`。没有 I/O，没有副作用，不是脚本运行时。

Deterministic formula parser and evaluator. Same source + same bindings → same `f64`. No I/O, no side effects.

天枢桌面工作台用它算数值。**本仓库只有公式 crate**，不含工作台、Play、节点图或文档 IR。

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

## Install

```toml
[dependencies]
tianshu-formula = { git = "https://github.com/tianshu48/tianshu-formula" }
```

尚未发 crates.io。

## Usage

```rust
use tianshu_formula::{Formula, MapContext};

fn main() -> Result<(), tianshu_formula::Error> {
    let f = Formula::parse("clamp(atk * 1.5 + flat, 0, 9999)")?;
    let mut ctx = MapContext::new();
    ctx.insert("atk", 100.0).insert("flat", 20.0);
    assert_eq!(f.eval(&ctx)?, 170.0);
    Ok(())
}
```

热路径是同步 `eval`。`eval_async` / `eval_batch_async` 只是同一套求值的 async 包装。

## Not this

| 不是 | 实际是 |
|------|--------|
| Excel / 电子表格 | 单行表达式，变量来自 `Context` |
| 脚本语言（循环、赋值、I/O） | 纯表达式；求值不写环境 |
| 天枢 Play / 节点图 | 只产出 `f64` |
| 任意精度 / 十进制财务 | IEEE `f64` |

`NaN`、除零、取模零都是 **错误**，不会静默变成 `NaN`。

## Language

### Literals and names

| 形式 | 例子 |
|------|------|
| 数字 | `42` `3.14` `.5` `1e-3` |
| 变量 | `atk`；点号是**完整键名** `foo.bar`，不是字段访问 |
| 常量 | `pi` `e` `tau` `inf` |

### Operators (high → low)

1. atom：字面量、变量、`(…)`、`name(args…)`
2. unary：`+` `-` `!`（非零为真）
3. `^`（右结合）
4. `* / %`
5. `+ -`
6. `< <= > >= == !=` → `1.0` / `0.0`
7. `&&`（短路）
8. `||`（短路）
9. `cond ? a : b`（短路，右结合）

### Built-in functions

`abs` `sign` `floor` `ceil` `round` `trunc` `fract`  
`min` `max` `sum` `clamp`  
`sqrt` `cbrt` `pow` `hypot` `exp` `ln` `log` `log2` `log10`  
`sin` `cos` `tan` `asin` `acos` `atan` `atan2`  
`deg` `rad`  
`if` `select`  
`lerp` `smoothstep` `step`

内建名不能经 `Registry` 覆盖（解析期 `BuiltinOverride`）。函数表见 [`src/builtins.rs`](src/builtins.rs)。

AST 嵌套上限 [`MAX_AST_DEPTH`](src/lib.rs)（256）。超出为 `Parse`。

## API

```rust
Formula::parse(src)
Formula::parse_with(src, Some(&registry))
formula.eval(&ctx)
formula.eval_with(&ctx, Some(&registry))
formula.eval_batch(&contexts)
formula.eval_batch_into(&contexts, &mut out)
formula.eval_async(&ctx).await          // 包装 sync eval
formula.eval_batch_async(&contexts).await
```

`Formula` 可 `Serialize` / `Deserialize`（AST + `version`，当前 `FORMULA_VERSION = 1`）。

`Context` 自己实现即可；`MapContext` 是 HashMap 适配。

## Custom functions

```rust
use std::sync::Arc;
use tianshu_formula::{Formula, MapContext, Registry};

let mut reg = Registry::new();
reg.register(
    "double",
    Arc::new(|args| {
        if args.len() != 1 {
            return Err(tianshu_formula::Error::new(
                tianshu_formula::ErrorKind::Arity,
                "double expects 1 arg",
            ));
        }
        Ok(args[0] * 2.0)
    }),
)?;
let f = Formula::parse_with("double(3)", Some(&reg))?;
assert_eq!(f.eval_with(&MapContext::new(), Some(&reg))?, 6.0);
```

自定义函数必须在 **parse 和 eval 时传入同一个 `Registry`**。只 parse 不传、eval 才传，会在 parse 时报 `UnknownFunction`。

## Errors

`Error.kind`：

`EmptyInput` `Lex` `Parse` `UnknownFunction` `Arity` `UndefinedVariable` `DivByZero` `Domain` `Nan` `BuiltinOverride` `Custom`

可选 `span`、`name`。

## Develop

```bash
cargo test
cargo bench
```

## License

[MIT](LICENSE) © 天枢
