# la_template_rs

Language-agnostic template implemented in Rust

Template:

```bash
hello ${world_name}, this is ${name} reporting. The total cost is \$12.
```

Given some var.json

```json
{
    "world_name": "world",
    "name": "pegasust"
}
```

This results in:

```text
hello world, this is pegasust reporting. The total cost is $12.
```
