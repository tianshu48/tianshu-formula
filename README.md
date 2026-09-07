# tianshu-formula

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

## 安装

```toml
[dependencies]
tianshu-formula = { git = "https://github.com/tianshu48/tianshu-formula" }
```

还没发 crates.io。

## 用法

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

常用路径是同步 `eval`。`eval_async` 和 `eval_batch_async` 内部仍是同一套求值。

## 范围

| 容易当成 | 这里实际是 |
|------|--------|
| Excel / 电子表格 | 单行表达式，变量来自 `Context` |
| 脚本语言（循环、赋值、I/O） | 纯表达式；求值不写环境 |
| 天枢 Play / 节点图 | 只产出 `f64` |
| 任意精度 / 十进制财务 | IEEE `f64` |

`NaN`、除零、取模零会返回错误，不会变成 `NaN` 继续算。

## 语言

### 字面量和名字

| 形式 | 例子 |
|------|------|
| 数字 | `42` `3.14` `.5` `1e-3` |
| 变量 | `atk`；点号算完整键名（`foo.bar`），不是取字段 |
| 常量 | `pi` `e` `tau` `inf` |

### 运算符（从高到低）

1. atom：字面量、变量、`(…)`、`name(args…)`
2. unary：`+` `-` `!`（非零为真）
3. `^`（右结合）
4. `* / %`
5. `+ -`
6. `< <= > >= == !=` 结果为 `1.0` / `0.0`
7. `&&`（短路）
8. `||`（短路）
9. `cond ? a : b`（短路，右结合）

### 内建函数

`abs` `sign` `floor` `ceil` `round` `trunc` `fract`  
`min` `max` `sum` `clamp`  
`sqrt` `cbrt` `pow` `hypot` `exp` `ln` `log` `log2` `log10`  
`sin` `cos` `tan` `asin` `acos` `atan` `atan2`  
`deg` `rad`  
`if` `select`  
`lerp` `smoothstep` `step`

内建名不能经 `Registry` 覆盖，解析时是 `BuiltinOverride`。函数表见 [`src/builtins.rs`](src/builtins.rs)。

AST 嵌套上限 [`MAX_AST_DEPTH`](src/lib.rs)（256）。超出返回 `Parse`。

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

`Formula` 可以 `Serialize` / `Deserialize`（AST 加 `version`，当前 `FORMULA_VERSION = 1`）。

可以自己实现 `Context`；`MapContext` 是 HashMap 适配。

## 自定义函数

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

parse 和 eval 要传同一个 `Registry`。parse 时没传、只在 eval 时传，parse 会报 `UnknownFunction`。

## 错误

`Error.kind`：

`EmptyInput` `Lex` `Parse` `UnknownFunction` `Arity` `UndefinedVariable` `DivByZero` `Domain` `Nan` `BuiltinOverride` `Custom`

`span` 和 `name` 是可选字段。

## 开发

```bash
cargo test
cargo bench
```

## 许可

[MIT](LICENSE) © 天枢
