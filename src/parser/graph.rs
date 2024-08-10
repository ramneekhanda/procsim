use std::collections::{HashMap, HashSet};

type GraphType = HashMap<String, HashSet<String>>;

pub fn parse_graph(graph_code: &String) -> Result<GraphType, String> {
    let lines = graph_code.lines();
    let mut nodes = GraphType::new();

    for (i, mut line) in lines.enumerate() {
        line = line.trim();
        if line.len() > 0 {
            let splits: Vec<_> = line.split(":").collect();
            println!("{:?}", splits);
            if splits.len() != 2 {
                return Err("ill format line ".to_string() + &i.to_string());
            } else {
                let deps: Vec<String> = splits
                    .get(1)
                    .unwrap()
                    .split(' ')
                    .map(|x| x.to_string())
                    .collect();
                let deps_clean: HashSet<String> = deps
                    .iter()
                    .map(|x| x.trim().to_string())
                    .filter(|x| x.len() > 0)
                    .collect::<HashSet<_>>();
                let key = splits.get(0).unwrap().trim().to_string();

                // insert reverse map
                for s in deps_clean.iter() {
                    if *s != key {
                        if nodes.contains_key(s) {
                            nodes.get_mut(s).unwrap().insert(key.clone());
                        } else {
                            nodes.insert(s.clone(), HashSet::from_iter([key.clone()]));
                        }
                    }
                }

                if nodes.contains_key(&key) {
                    for dep in deps_clean.iter() {
                        nodes.get_mut(&key).unwrap().insert(dep.clone());
                    }
                } else {
                    nodes.insert(key, HashSet::from_iter(deps_clean));
                }
            }
        }
    }
    return Ok(nodes);
}

#[test]
fn parse_graph_test() {
    let code = String::from(
        "a: b c d e f
                           b: g
                           g: d",
    );
    let res = parse_graph(&code);
    assert!(res.is_ok());
    let hm = res.ok().unwrap();

    assert!(hm.get("a").unwrap().len() == 5, "{:?}", hm);
    assert!(hm.get("b").unwrap().len() == 2, "{:?}", hm);
    assert!(hm.get("c").unwrap().len() == 1, "{:?}", hm);
    assert!(hm.get("d").unwrap().len() == 2, "{:?}", hm);
    assert!(hm.get("e").unwrap().len() == 1, "{:?}", hm);
    assert!(hm.get("f").unwrap().len() == 1, "{:?}", hm);
    assert!(hm.get("g").unwrap().len() == 2, "{:?}", hm);
}

#[test]
fn parse_graph_test_dup_removal() {
    let code = String::from(
        "a: b b
                           b: a
              ",
    );
    let res = parse_graph(&code);
    assert!(res.is_ok());
    let hm = res.ok().unwrap();

    assert!(hm.get("a").unwrap().len() == 1, "{:?}", hm);
}
