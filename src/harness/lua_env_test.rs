use itertools::Itertools;
use std::collections::HashMap;

use mlua::prelude::*;
use mlua::{FromLua, Lua, MetaMethod, Result, UserData, UserDataMethods, Value};

#[derive(Clone, Debug, PartialEq)]
struct EnvList {
    list: HashMap<String, Vec<String>>,
}

impl EnvList {
    fn from(map: HashMap<String, Vec<String>>) -> Self {
        EnvList {
            list: HashMap::from(map),
        }
    }
}

impl FromLua for EnvList {
    fn from_lua(value: Value, _: &Lua) -> Result<Self> {
        match value {
            Value::UserData(ud) => Ok(ud.borrow::<Self>()?.clone()),
            _ => unreachable!(),
        }
    }
}

impl UserData for EnvList {
    // union
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_function(MetaMethod::Add, |_, (lhs, rhs): (EnvList, EnvList)| {
            assert_eq!(
                lhs.list.keys().sorted().collect_vec(),
                rhs.list.keys().sorted().collect_vec()
            );

            let mut env_union = lhs.clone();
            env_union.list.extend(rhs.list);
            env_union.list.values_mut().for_each(|v| {
                v.dedup();
                v.sort();
            });

            Ok(env_union)
        });
    }
}

fn evaluate_env_lua() -> LuaResult<Vec<EnvList>> {
    let lua = Lua::new();
    let globals = lua.globals();

    // (1) creation
    // create a set of values from a list
    let from_list = lua.create_function(|_, (variable, values): (String, Vec<String>)| {
        let env = EnvList::from(HashMap::from([(variable, values)]));

        Ok(env)
    })?;
    globals.set("from_list", from_list)?;

    // create a set of values from a newline seperated string
    let from_output = lua.create_function(|_, (variable, value): (String, String)| {
        let env = EnvList::from(HashMap::from([(
            variable,
            value.split("\n").map(|s| s.to_string()).collect(),
        )]));
        Ok(env)
    })?;
    globals.set("from_output", from_output)?;

    // (2) mutation
    // create the union of sets (only sets with equal keys)
    let cross_prod = lua.create_function(|_, lists: Vec<EnvList>| {
        // TODO
        Ok(lists)
    })?;
    globals.set("cross", cross_prod)?;

    lua.load(std::fs::read_to_string("tests/env_test.lua").expect("no file at this location"))
        .eval()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn call_eval() {
        let res = evaluate_env_lua();
        print!("{res:?}\n");

        assert!(res.is_ok())
    }
}
