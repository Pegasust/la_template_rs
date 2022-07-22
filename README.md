# la_template_rs

A wrapper of `la_template_base` that manages target creation and vars

## What it does

Given some templates
```bash
# hello_world.t.txt
hello ${world_name}, this is ${name} reporting. The totalcost is \$12.
# bye_world.t.txt
bye ${world_name}.
```


Some conforming var declarations:
```json
// pegasust.json
{"world_name": "world", "name": "pegasust"}
// hungtr-uofa.json
{"world_name": "U of A", "name": "hungtr-uofa"}
```

This application manages the template to their vars

```json
{
    "vars": [
        {"var": "pegasust.json", "metadata": {"target": ""}},
        {"var": "hungtr-uofa.json", "metadata": {"target": "uofa"}}
    ],
    "templates": ["hello_world.t.txt", "bye_world.t.txt"],
    // uses Regex::replace(pattern, format!(replace))
    "replace_regex": {
        "pattern": "(.t)",
        // more info here https://docs.rs/regex/1.1.0/regex/struct.Regex.html#method.replace
        // Some variables that are available in the scope for format! (powered by crates.io/strfmt)
        "replace": "{target}"
    }
}
```
## TODO

- [x] Implement rough template_fs
- [x] cf_fs: Categorize NaiveFS and RootedFS to PathInterpreter
- [ ] cf_fs: Implicit absolute path converter for PathInterpreter?
- [ ] cf_fs: FileImpl should implement Read
- [ ] la_template_rs: Implement main.rs. This provides a good overlook for resulting API
- [ ] Unit test for replace_regex.rs
- [ ] la_template_rs: Integration test using Docker container and copy
- [ ] la_template_rs: Integration test using MemFS
- [ ] template_fs: Test MemFS
- [ ] template_fs: Test NaiveFS
- [ ] template_fs: Test RootedFS
- [ ] template_fs: Implement PathMappedFS
- [ ] template_fs: Test PathMappedFS
