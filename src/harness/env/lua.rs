use itertools::Itertools;
use std::collections::HashMap;

use mlua::prelude::*;
use mlua::{FromLua, Lua, MetaMethod, Result, UserData, UserDataMethods, Value};

use crate::harness::env::EnvList;

#[derive(Clone, Debug, PartialEq)]
struct LuaEnvList {
    list: HashMap<String, Vec<String>>,
}

impl LuaEnvList {
    fn from(map: HashMap<String, Vec<String>>) -> Self {
        LuaEnvList {
            list: HashMap::from(map),
        }
    }
}

impl FromLua for LuaEnvList {
    fn from_lua(value: Value, _: &Lua) -> Result<Self> {
        // helper to read an EnvList from a Lua Table
        fn envlist_from_lua(tb: LuaTable) -> Result<LuaEnvList> {
            let mut map = HashMap::new();
            for pair in tb.pairs::<String, mlua::Table>() {
                let (key, val_table) = pair?;
                let vec = val_table
                    .sequence_values::<String>()
                    .collect::<Result<Vec<_>>>()?;
                map.insert(key, vec);
            }

            Ok(LuaEnvList::from(map))
        }

        match value {
            Value::UserData(ud) => Ok(ud.borrow::<Self>()?.clone()),
            Value::Table(tb) => envlist_from_lua(tb),
            Value::Nil => Ok(LuaEnvList::from(HashMap::new())),
            _ => Err(LuaError::ToLuaConversionError {
                from: value.to_string().unwrap(),
                to: "LuaEnvList",
                message: None,
            }),
        }
    }
}

impl UserData for LuaEnvList {
    // Add multiple EnvLists to a List of EnvLists with "+"
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_function(
            MetaMethod::Add,
            |_, (lhs, rhs): (LuaEnvList, LuaEnvList)| {
                if lhs.list.keys().sorted().collect_vec() != rhs.list.keys().sorted().collect_vec()
                {
                    Err(LuaError::external("Key missmatch"))
                } else {
                    Ok(vec![lhs.clone(), rhs.clone()])
                }
            },
        );
    }
}

pub fn eval(chunk_str: String) -> LuaResult<Vec<EnvList>> {
    let lua = Lua::new();
    let globals = lua.globals();

    // Register EnvList as a Lua userdata type
    lua.register_userdata_type::<LuaEnvList>(|metatable| {
        LuaEnvList::add_methods(metatable);
    })?;

    // (1) creation
    // create a set of values from a list with "from_list()"
    let from_list = lua.create_function(|_, (variable, values): (String, Vec<String>)| {
        let env = LuaEnvList::from(HashMap::from([(variable, values)]));

        Ok(env)
    })?;
    globals.set("from_list", from_list)?;

    // create a set of values from a newline seperated string with "from_output()"
    let from_output = lua.create_function(|_, (variable, value): (String, String)| {
        let env = LuaEnvList::from(HashMap::from([(
            variable,
            value.split("\n").map(|s| s.to_string()).collect(),
        )]));
        Ok(env)
    })?;
    globals.set("from_output", from_output)?;

    // (2) mutation
    // create the union of all provided EnvLists with "cross()"
    let cross_prod = lua.create_function(|_, lists: Vec<LuaEnvList>| {
        let mut combined = LuaEnvList::from(HashMap::new());

        for env in lists {
            for (key, values) in env.list {
                combined
                    .list
                    .entry(key)
                    .or_insert_with(Vec::new)
                    .extend(values);
            }
        }

        combined.list.values_mut().for_each(|v| {
            v.sort();
            v.dedup();
        });

        Ok(combined)
    })?;
    globals.set("cross", cross_prod)?;

    // (3) load and evaluate
    // Try to evaluate as Vec<EnvList>, if value is no table: fallback to EnvList
    let chunk = lua.load(&chunk_str);
    let lua_env = match chunk.eval::<Vec<LuaEnvList>>() {
        Ok(vec) => Ok(vec),
        Err(_) => {
            let chunk = lua.load(&chunk_str);
            match chunk.eval::<LuaEnvList>() {
                Ok(single) => Ok(vec![single]),
                Err(e) => Err(e),
            }
        }
    }?;

    let lua_env: Vec<EnvList> = lua_env.iter().map(|env| env.list.clone()).collect();
    Ok(lua_env)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_lua() {
        let empty = String::new();
        let res = eval(empty).unwrap();

        assert_eq!(res.len(), 1);
        assert!(res[0].is_empty())
    }

    #[test]
    fn invalid_lua() {
        let invalid = String::from("this in not lua I don't know what else to write");
        assert!(eval(invalid).is_err())
    }

    #[test]
    fn lua_from_file() {
        let chunk_src = std::fs::read_to_string("tests/env_test.lua").unwrap();
        assert!(eval(chunk_src).is_ok())
    }

    #[test]
    fn lua_cross() {
        let chunk_src = String::from(
            "freqs = from_list(\"FREQ\", {1000, 2000, 3000})
kernels = from_output(\"KERNELS\", \"add\\nmul\\ndiv\")
cpus = from_list(\"CPUS\", {\"0,1\", \"0,1,2,3\"})
result = cross({freqs, cpus, kernels})
return result",
        );
        let res = eval(chunk_src).unwrap();
        assert_eq!(res.len(), 1);

        let envlist = &res[0];
        assert_eq!(envlist.len(), 3);
        assert_eq!(envlist.get("FREQ").unwrap(), &vec!["1000", "2000", "3000"]);
        assert_eq!(envlist.get("KERNELS").unwrap(), &vec!["add", "div", "mul"]);
        assert_eq!(envlist.get("CPUS").unwrap(), &vec!["0,1", "0,1,2,3"]);
    }

    #[test]
    fn lua_union() {
        let chunk_src = String::from(
            "freqs = from_list(\"FREQ\", {1000, 2000, 3000})
result = freqs + freqs
return result",
        );
        let res = eval(chunk_src).unwrap();
        assert_eq!(res.len(), 2);

        let envlist = &res[0];
        assert_eq!(envlist.len(), 1);
        assert_eq!(envlist.get("FREQ").unwrap(), &vec!["1000", "2000", "3000"]);

        let envlist = &res[1];
        assert_eq!(envlist.len(), 1);
        assert_eq!(envlist.get("FREQ").unwrap(), &vec!["1000", "2000", "3000"]);
    }

    #[test]
    fn lua_union_key_missmatch() {
        let chunk_src = String::from(
            "freqs = from_list(\"FREQ\", {1000, 2000, 3000})
kernels = from_output(\"KERNELS\", \"add\\nmul\\ndiv\")
result = freqs + kernels
return result",
        );
        assert!(eval(chunk_src).is_err());
    }

    #[test]
    fn lua_full() {
        let chunk_str = String::from("freqs = from_list(\"FREQ\", {1000, 2000, 3000})
kernels = from_output(\"KERNELS\", \"add\\nmul\\ndiv\")
cpus = from_list(\"CPUS\", {\"0,1\", \"0,1,2,3\"})
result = cross({freqs, cpus, kernels, from_list(\"TURBO\", {\"OFF\"})}) + cross({from_list(\"FREQ\", {3000}), cpus, kernels, from_list(\"TURBO\", {\"ON\"})})
return result");

        let res = eval(chunk_str).unwrap();
        assert_eq!(res.len(), 2);

        let envlist = &res[0];
        assert_eq!(envlist.len(), 4);
        assert_eq!(envlist.get("FREQ").unwrap(), &vec!["1000", "2000", "3000"]);
        assert_eq!(envlist.get("KERNELS").unwrap(), &vec!["add", "div", "mul"]);
        assert_eq!(envlist.get("CPUS").unwrap(), &vec!["0,1", "0,1,2,3"]);
        assert_eq!(envlist.get("TURBO").unwrap(), &vec!["OFF",]);

        let envlist = &res[1];
        assert_eq!(envlist.len(), 4);
        assert_eq!(envlist.get("FREQ").unwrap(), &vec!["3000"]);
        assert_eq!(envlist.get("KERNELS").unwrap(), &vec!["add", "div", "mul"]);
        assert_eq!(envlist.get("CPUS").unwrap(), &vec!["0,1", "0,1,2,3"]);
        assert_eq!(envlist.get("TURBO").unwrap(), &vec!["ON",]);
    }
}
