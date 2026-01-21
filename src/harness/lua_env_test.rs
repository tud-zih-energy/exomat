use mlua::prelude::*;
use mlua::{FromLua, Lua, MetaMethod, Result, UserData, UserDataMethods, Value};

use crate::harness::env::{EnvList, Environment, EnvironmentContainer};

impl UserData for Environment {}

fn evaluate_env_lua() -> LuaResult<()> {
    let lua = Lua::new();
    let globals = lua.globals();

    // create a set of values from a list
    let from_list = lua.create_function(|_, (variable, values): (String, Vec<String>)| {
        let env = EnvList::from([(variable, values)]);
        Ok(env)
    })?;
    globals.set("from_list", from_list)?;

    // create a set of values from a newline seperated string
    let from_output = lua.create_function(|_, (variable, value): (String, String)| {
        let env = EnvList::from([(variable, value.split("\n").map(|s| s.to_string()).collect())]);
        Ok(env)
    })?;
    globals.set("from_output", from_output)?;

    // create the union of sets
    let union_of = lua.create_function(|_, (list1, list2): (Vec<String>, Vec<String>)| {
        // TODO
        Ok(())
    })?;
    globals.set("union_of", union_of)?;

    // create the cross product of sets
    let cross_product_of =
        lua.create_function(|_, (list1, list2): (Vec<String>, Vec<String>)| {
            // TODO
            Ok(())
        })?;
    globals.set("cross_product_of", cross_product_of)?;

    Ok(())
}
