use {HasLua, CopyRead, ConsumeRead, LoadedVariable, Push, Index};
use ffi;

#[unstable]
pub struct LuaTable<'var, L: 'var> {
    variable: LoadedVariable<'var, L>
}

impl<'var, L: HasLua> HasLua for LuaTable<'var, L> {
    fn use_lua(&mut self) -> *mut ffi::lua_State {
        self.variable.use_lua()
    }
}

// while the LuaTableIterator is active, the current key is constantly pushed over the table
#[unstable]
pub struct LuaTableIterator<'var: 'table, 'table, L: 'var> {
    table: &'table mut LuaTable<'var, L>,
    finished: bool,     // if true, the key is not on the stack anymore
}

impl<'var, 'table, L: HasLua> HasLua for LuaTableIterator<'var, 'table, L> {
    fn use_lua(&mut self) -> *mut ffi::lua_State {
        self.table.use_lua()
    }
}

impl<'var, L: HasLua> ConsumeRead<'var, L> for LuaTable<'var, L> {
    fn read_from_variable(mut var: LoadedVariable<'var, L>)
        -> Result<LuaTable<'var, L>, LoadedVariable<'var, L>>
    {
        if unsafe { ffi::lua_istable(var.use_lua(), -1) } {
            Ok(LuaTable{ variable: var })
        } else {
            Err(var)
        }
    }
}

impl<'var, L: HasLua> LuaTable<'var, L> {
    pub fn iter<'me>(&'me mut self)
        -> LuaTableIterator<'var, 'me, L>
    {
        unsafe { ffi::lua_pushnil(self.variable.use_lua()) };
        LuaTableIterator{table: self, finished: false}
    }

    pub fn load<'a, R: ConsumeRead<'a, LuaTable<'var, L>>, I: Index<LuaTable<'var, L>>>(&'a mut self, index: I) -> Option<R> {
        index.push_to_lua(self);
        unsafe { ffi::lua_gettable(self.use_lua(), -2); }
        let var = LoadedVariable{lua: self, size: 1};
        ConsumeRead::read_from_variable(var).ok()
    }

    pub fn load_table<'a, I: Index<LuaTable<'var, L>>>(&'a mut self, index: I) -> Option<LuaTable<'a, LuaTable<'var, L>>> {
        self.load(index)
    }

    pub fn get<R: CopyRead<LuaTable<'var, L>>, I: Index<LuaTable<'var, L>>>(&mut self, index: I) -> Option<R> {
        index.push_to_lua(self);
        unsafe { ffi::lua_gettable(self.use_lua(), -2); }
        let value = CopyRead::read_from_lua(self, -1);
        unsafe { ffi::lua_pop(self.use_lua(), 1); }
        value
    }

    pub fn set<I: Index<LuaTable<'var, L>>, V: Push<LuaTable<'var, L>>>(&mut self, index: I, value: V) {
        index.push_to_lua(self);
        value.push_to_lua(self);
        unsafe { ffi::lua_settable(self.use_lua(), -3); }
    }

    // Obtains or create the metatable of the table
    pub fn get_or_create_metatable(mut self) -> LuaTable<'var, L> {
        let result = unsafe { ffi::lua_getmetatable(self.variable.use_lua(), -1) };

        if result == 0 {
            unsafe {
                ffi::lua_newtable(self.variable.use_lua());
                ffi::lua_setmetatable(self.variable.use_lua(), -2);
                let r = ffi::lua_getmetatable(self.variable.use_lua(), -1);
                assert!(r != 0);
            }
        }

        // note: it would be cleaner to create another table, but cannot manage to make it compile
        self.variable.size += 1;
        self
    }
}

impl<'a, 'b, L: HasLua, K: CopyRead<LuaTableIterator<'a, 'b, L>>, V: CopyRead<LuaTableIterator<'a, 'b, L>>>
    Iterator<Option<(K, V)>> for LuaTableIterator<'a, 'b, L>
{
    fn next(&mut self)
        -> Option<Option<(K,V)>>
    {
        if self.finished {
            return None
        }

        // this call pushes the next key and value on the stack
        if unsafe { ffi::lua_next(self.table.use_lua(), -2) } == 0 {
            self.finished = true;
            return None
        }

        let key = CopyRead::read_from_lua(self, -2);
        let value = CopyRead::read_from_lua(self, -1);

        // removing the value, leaving only the key on the top of the stack
        unsafe { ffi::lua_pop(self.table.use_lua(), 1) };

        //
        if key.is_none() || value.is_none() {
            Some(None)
        } else {
            Some(Some((key.unwrap(), value.unwrap())))
        }
    }
}

#[unsafe_destructor]
impl<'a, 'b, L: HasLua> Drop for LuaTableIterator<'a, 'b, L> {
    fn drop(&mut self) {
        if !self.finished {
            unsafe { ffi::lua_pop(self.table.variable.use_lua(), 1) }
        }
    }
}
