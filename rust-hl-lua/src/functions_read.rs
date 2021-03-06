use ffi;
use std::io::IoError;
use {HasLua, ConsumeRead, CopyRead, LoadedVariable, LuaError, ExecutionError, WrongType, ReadError, SyntaxError};

#[unstable]
///
/// Lifetime `'a` represents the lifetime of the function on the stack.
/// Param `L` represents the stack the function has been loaded on and must be a `HasLua`.
pub struct LuaFunction<'a, L: 'a> {
    variable: LoadedVariable<'a, L>,
}

struct ReadData {
    reader: Box<Reader + 'static>,
    buffer: [u8, ..128],
    triggered_error: Option<IoError>,
}

extern fn reader(_: *mut ffi::lua_State, data_raw: *mut ::libc::c_void, size: *mut ::libc::size_t) -> *const ::libc::c_char {
    use std::io::EndOfFile;

    let data: &mut ReadData = unsafe { ::std::mem::transmute(data_raw) };

    if data.triggered_error.is_some() {
        unsafe { (*size) = 0 }
        return data.buffer.as_ptr() as *const ::libc::c_char;
    }

    match data.reader.read(data.buffer.as_mut_slice()) {
        Ok(len) =>
            unsafe { (*size) = len as ::libc::size_t },
        Err(ref e) if e.kind == EndOfFile =>
            unsafe { (*size) = 0 },
        Err(e) => {
            unsafe { (*size) = 0 }
            data.triggered_error = Some(e)
        },
    };

    data.buffer.as_ptr() as *const ::libc::c_char
}

impl<'a, L: HasLua> LuaFunction<'a, L> {
    pub fn call<V: CopyRead<LoadedVariable<'a, L>>>(&mut self) -> Result<V, LuaError> {
        // calling pcall pops the parameters and pushes output
        let pcall_return_value = unsafe { ffi::lua_pcall(self.variable.use_lua(), 0, 1, 0) };     // TODO:

        // if pcall succeeded, returning
        if pcall_return_value == 0 {
            return match CopyRead::read_from_lua(&mut self.variable, -1) {
                None => Err(WrongType),
                Some(x) => Ok(x)
            };
        }

        // an error occured during execution
        if pcall_return_value == ffi::LUA_ERRMEM {
            panic!("lua_pcall returned LUA_ERRMEM");
        }

        if pcall_return_value == ffi::LUA_ERRRUN {
            let error_msg: String = CopyRead::read_from_lua(self.variable.lua, -1).expect("can't find error message at the top of the Lua stack");
            unsafe { ffi::lua_pop(self.variable.use_lua(), 1) };
            return Err(ExecutionError(error_msg));
        }

        panic!("Unknown error code returned by lua_pcall: {}", pcall_return_value)
    }

    pub fn load_from_reader<R: ::std::io::Reader + 'static>(lua: &'a mut L, code: R)
        -> Result<LuaFunction<'a, L>, LuaError>
    {
        let readdata = ReadData {
            reader: box code,
            buffer: unsafe { ::std::mem::uninitialized() },
            triggered_error: None,
        };

        let load_return_value = "chunk".with_c_str(|chunk|
            unsafe { ffi::lua_load(lua.use_lua(), reader, ::std::mem::transmute(&readdata), chunk, ::std::ptr::null()) }
        );

        if readdata.triggered_error.is_some() {
            let error = readdata.triggered_error.unwrap();
            return Err(ReadError(error));
        }

        if load_return_value == 0 {
            return Ok(LuaFunction{
                variable: LoadedVariable{
                    lua: lua,
                    size: 1
                }
            });
        }

        let error_msg: String = CopyRead::read_from_lua(lua, -1).expect("can't find error message at the top of the Lua stack");
        unsafe { ffi::lua_pop(lua.use_lua(), 1) };

        if load_return_value == ffi::LUA_ERRMEM {
            panic!("LUA_ERRMEM");
        }
        if load_return_value == ffi::LUA_ERRSYNTAX {
            return Err(SyntaxError(error_msg));
        }

        panic!("Unknown error while calling lua_load");
    }

    pub fn load(lua: &'a mut L, code: &str)
        -> Result<LuaFunction<'a, L>, LuaError>
    {
        let reader = ::std::io::MemReader::new(code.to_c_str().as_bytes().init().to_vec());
        LuaFunction::load_from_reader(lua, reader)
    }
}

// TODO: return Result<Ret, ExecutionError> instead
/*impl<'a, 'lua, Ret: CopyRead> ::std::ops::FnMut<(), Ret> for LuaFunction<'a,'lua> {
    fn call_mut(&mut self, _: ()) -> Ret {
        self.call().unwrap()
    }
}*/

impl<'a, L: HasLua> ConsumeRead<'a, L> for LuaFunction<'a, L> {
    fn read_from_variable(mut var: LoadedVariable<'a, L>)
        -> Result<LuaFunction<'a, L>, LoadedVariable<'a, L>>
    {
        if unsafe { ffi::lua_isfunction(var.use_lua(), -1) } {
            Ok(LuaFunction{ variable: var })
        } else {
            Err(var)
        }
    }
}
