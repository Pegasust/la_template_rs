use cf_fs::*;
use itertools::Itertools;
use std::{collections::HashMap};

#[test]
fn single_forward() {
    let relative = cf_fs::ForwardPath::default()
        .output("proper/relative/path")
        .expect("proper relative should not be err");
    assert_eq!(relative.as_ref(), "proper/relative/path");
    let absolute = cf_fs::ForwardPath::default()
        .output("/this/is/absolute")
        .expect("absolute should not be err");
    assert_eq!(absolute.as_ref(), "/this/is/absolute");
    let relative_dot = cf_fs::ForwardPath::default()
        .output("./a.out")
        .expect("current directory should not err");
    assert_eq!(relative_dot.as_ref(), "./a.out");
}

#[test]
fn bypass_absolute() {
    // any absolute path must be respected.
    let abs_str = "/home/ubuntu/hello.txt";

    let forward: PathPlugin = cf_fs::ForwardPath::default().into();
    let empty_remap: PathPlugin = PathRemap::default().into();
    let empty_suffix: PathPlugin = SuffixRelativePath::default().into();

    let some_remap: PathPlugin = PathRemap::new(
        HashMap::from_iter(vec!
            [
                (b"hello".to_vec(), b"world".to_vec()),
                (b"new phone".to_vec(), b"who dis".to_vec())
            ])
    ).into();

    let rel_suffix: PathPlugin = SuffixRelativePath::new("my/root/folder").into();
    let rel_suffix_trail: PathPlugin = SuffixRelativePath::new("my/root/folder/").into();

    assert_eq!(forward.output(abs_str).expect("Should not err"), abs_str);
    assert_eq!(empty_remap.output(abs_str).expect("Should not err"), abs_str);
    assert_eq!(empty_suffix.output(abs_str).expect("Should not err"), abs_str);
    assert_eq!(some_remap.output(abs_str).expect("Should not err"), abs_str);
    assert_eq!(rel_suffix.output(abs_str).expect("Should not err"), abs_str);
    assert_eq!(rel_suffix_trail.output(abs_str).expect("Should not err"), abs_str);
}

#[test]
fn bypass_composition() {
    // Composition of plugins that are bypassed
    // will also be bypassed

    let abs_str = "/home/ubuntu/hello.txt";

    let forward: PathPlugin = cf_fs::ForwardPath::default().into();

    let some_remap: PathPlugin = PathRemap::new(
        HashMap::from_iter(vec!
            [
                (b"hello".to_vec(), b"world".to_vec()),
                (b"new phone".to_vec(), b"who dis".to_vec())
            ])
    ).into();

    let rel_suffix: PathPlugin = SuffixRelativePath::new("my/root/folder").into();
    let rel_suffix_trail: PathPlugin = SuffixRelativePath::new("my/root/folder/").into();

    let plugins = vec![forward, some_remap, rel_suffix, rel_suffix_trail];

    for tup in plugins.iter().permutations(plugins.len()) {
        let output = PathInterpreter::new(tup.iter())
            .output(abs_str).expect("Should not err");
        assert_eq!(output, abs_str);
    }
        
}

#[test]
fn remap() {
    let some_remap: PathPlugin = PathRemap::new(
        HashMap::from_iter(vec!
            [
                (b"hello".to_vec(), b"world".to_vec()),
                (b"new_phone".to_vec(), b"who_dis".to_vec()),
                (b"my_number_is".to_vec(), b"123456-789".to_vec())
            ])
    ).into();
    // Note: We're not nitpicky here. We require that PathInterpreter
    // shall eliminate duplicated path splitter; any PathPlugin may or may not
    // eliminate path splitter.
    let interp = PathInterpreter::from(some_remap);
    let output_res = |abs_str| interp.output(abs_str);
    let output_str = |abs_str| output_res(abs_str).unwrap();
    assert_eq!(output_str("relative//no/remap"), "relative/no/remap");
    assert_eq!(output_str("@hello/src/pages/index.tsx"), "world/src/pages/index.tsx");
    assert_eq!(output_str("@new_phone"), "who_dis");
    assert!(matches!(output_res("/should/use/suffix/plugin/after/remap/@my_number_is"),
        Err(_)));
    assert!(matches!(output_res("@err/on/undefined/ref"), Err(_)));
    assert!(matches!(output_res("@err_on_singleton_path"), Err(_)));
}

#[test]
fn remap_then_reroot() {
    let some_remap: PathPlugin = PathRemap::new(
        HashMap::from_iter(vec!
            [
                (b"hello".to_vec(), b"world".to_vec()),
                (b"new_phone".to_vec(), b"who_dis".to_vec()),
                (b"my_number_is".to_vec(), b"123456-789".to_vec())
            ])
    ).into();
    let rel_prefix: PathPlugin = SuffixRelativePath::new("relative/path").into();
    let abs_prefix: PathPlugin = SuffixRelativePath::new("/absolute/path/").into();

    let rel = PathInterpreter::from(some_remap);
    let abs = rel.clone().then(abs_prefix);
    let rel = rel.then(rel_prefix);

    let output = |interp: &PathInterpreter, path| interp.output(path);
    let output_str = |interp, path| output(interp, path).expect("Should produce output str");
    
    assert_eq!(output_str(&abs, "no_remap"), "/absolute/path/no_remap");
    assert_eq!(output_str(&rel, "no_remap"), "relative/path/no_remap");

    assert!(matches!(output(&abs, "remap/error/@hello"), Err(_)));
    assert!(matches!(output(&rel, "@unprovided/remap"), Err(_)));

    assert_eq!(output_str(&abs, "@new_phone"), "/absolute/path/who_dis");
    assert_eq!(output_str(&rel, "@my_number_is"), "relative/path/123456-789");

}