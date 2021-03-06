// Copyright (C) 1994-2015 Lua.org, PUC-Rio.
// Copyright (C) 2016 Ahmed Charles - acharles@outlook.com
// Distributed under the MIT License.
//    (See accompanying file LICENSE.txt or copy at
//          http://opensource.org/licenses/MIT)

// Lua stand-alone interpreter

extern crate libc;
extern crate lua_rs;

use lua_rs::ffi;


const LUA_PROMPT: &'static str = "> ";
const LUA_PROMPT2: &'static str = ">> ";

const LUA_INIT_VAR_NAME: &'static str = "=LUA_INIT";

const LUA_INITVARVERSION_NAME: &'static str = "=LUA_INIT_5_3";


/*
** stdin_is_tty detects whether the standard input is a 'tty' (that
** is, whether we're running lua interactively).
*/
#[cfg(unix)]
fn stdin_is_tty() -> bool { unsafe { libc::isatty(0) != 0 } }

#[cfg(not(unix))]
fn stdin_is_tty() -> bool { true }  /* assume stdin is a tty */


/*
** readline defines how to show a prompt and then read a line from
** the standard input.
** saveline defines how to "save" a read line in a "history".
*/
#[cfg(feature = "readline")]
extern crate readline;
#[cfg(feature = "readline")]
fn readline(prompt: &str) -> Option<String> {
    match readline::readline(prompt) {
        Ok(s) => Some(s),
        Err(_) => None,
    }
}
#[cfg(feature = "readline")]
fn saveline(line: &str) {
    match readline::add_history(line) {
        Ok(_) => (), Err(_) => (),
    }
}

#[cfg(not(feature = "readline"))]
fn readline(prompt: &str) -> Option<String> {
    use std::io::{self, Write};
    write!(io::stdout(), "{}", prompt).unwrap();
    io::stdout().flush().unwrap();  /* show prompt */
    let mut input = String::new();
    match io::stdin().read_line(&mut input) {  /* get line */
        Ok(_) => Some(input),
        Err(_) => None,
    }
}
#[cfg(not(feature = "readline"))]
fn saveline(_: &str) {}




static mut global_l: *mut ffi::lua::lua_State = 0 as *mut _;


/*
** Hook set by signal function to stop the interpreter.
*/
extern "C" fn stop(l: *mut ffi::lua::lua_State, _: *mut ffi::lua::lua_Debug) {
    unsafe { ffi::lua::lua_sethook(l, None, 0, 0); }  /* reset hook */
    let s = std::ffi::CString::new("interrupted!").unwrap();
    unsafe { ffi::lauxlib::luaL_error(l, s.as_ptr()); }
}


/*
** Function to be called at a C signal. Because a C signal cannot
** just change a Lua state (as there is no proper synchronization),
** this function only sets a hook that, when called, will stop the
** interpreter.
*/
extern "C" fn laction(i: libc::c_int) {
    unsafe {
        libc::signal(i, libc::SIG_DFL); /* if another SIGINT happens, terminate process */
        ffi::lua::lua_sethook(global_l, Some(stop),
            ffi::lua::LUA_MASKCALL | ffi::lua::LUA_MASKRET | ffi::lua::LUA_MASKCOUNT, 1);
    }
}


fn print_usage(badoption: &str) {
    use std::io::{self, Write};
    let progname = std::env::args().next().unwrap();
    write!(&mut io::stderr(), "{}: ", progname).unwrap();
    let ch = badoption.chars().nth(1).unwrap();
    if ch == 'e' || ch == 'l' {
        writeln!(&mut io::stderr(), "'{}' needs argument", badoption).unwrap();
    } else {
        writeln!(&mut io::stderr(), "unrecognized option '{}'", badoption).unwrap();
    }
    writeln!(&mut io::stderr(),
r#"usage: {} [options] [script [args]]
Available options are:
  -e stat  execute string 'stat'
  -i       enter interactive mode after executing 'script'
  -l name  require library 'name'
  -v       show version information
  -E       ignore environment variables
  --       stop handling options
  -        stop handling options and execute stdin"#, progname).unwrap();
}


/*
** Prints an error message, adding the program name in front of it
** (if include_name is true).
*/
fn message(msg: &str, include_name: bool) {
    use std::io::{self, Write};
    if include_name {
        write!(&mut io::stderr(), "{}: ", std::env::args().next().unwrap()).unwrap();
    }
    writeln!(&mut io::stderr(), "{}", msg).unwrap();
}


/*
** Check whether 'status' is not OK and, if so, prints the error
** message on the top of the stack. It assumes that the error object
** is a string, as it was either generated by Lua or by 'msghandler'.
*/
fn report(l: *mut ffi::lua::lua_State, status: libc::c_int, include_name: bool) -> libc::c_int {
    if status != ffi::lua::LUA_OK {
        let mut len: libc::size_t = 0;
        let msg = unsafe { ffi::lua::lua_tolstring(l, -1, &mut len) };
        let msg_slice = unsafe { std::slice::from_raw_parts(msg as *const u8, len as usize) };
        message(std::str::from_utf8(msg_slice).unwrap(), include_name);
        unsafe { ffi::lua::lua_pop(l, 1) };  /* remove message */
    }
    status
}


/*
** Message handler used to run all chunks
*/
fn msghandler_(l: *mut ffi::lua::lua_State) -> libc::c_int {
    let mut msg = unsafe { ffi::lua::lua_tostring(l, 1) };
    if msg.is_null() {  /* is error object not a string? */
        let s = std::ffi::CString::new("__tostring").unwrap();
        if unsafe { ffi::lauxlib::luaL_callmeta(l, 1, s.as_ptr()) } != 0 &&  /* does it have a metamethod */
                unsafe { ffi::lua::lua_type(l, -1) } == ffi::lua::LUA_TSTRING {  /* that produces a string? */
            return 1;  /* that is the message */
        } else {
            let s = std::ffi::CString::new("(error object is a %s value)").unwrap();
            msg = unsafe { ffi::lua::lua_pushfstring(l, s.as_ptr(), ffi::lauxlib::luaL_typename(l, 1)) };
        }
    }
    unsafe { ffi::lauxlib::luaL_traceback(l, l, msg, 1); }  /* append a standard traceback */
    1  /* return the traceback */
}

unsafe extern "C" fn msghandler(l: *mut ffi::lua::lua_State) -> libc::c_int {
    msghandler_(l)
}


/*
** Interface to 'lua_pcall', which sets appropriate message function
** and C-signal handler. Used to run all chunks.
*/
fn docall(l: *mut ffi::lua::lua_State, narg: libc::c_int, nres: libc::c_int) -> libc::c_int {
    let base = unsafe { ffi::lua::lua_gettop(l) } - narg;  /* function index */
    unsafe { ffi::lua::lua_pushcfunction(l, Some(msghandler)); }  /* push message handler */
    unsafe { ffi::lua::lua_insert(l, base); }  /* put it under function and args */
    unsafe { global_l = l; }  /* to be available to 'laction' */
    unsafe { libc::signal(libc::SIGINT, laction as libc::sighandler_t); }  /* set C-signal handler */
    let status = unsafe { ffi::lua::lua_pcall(l, narg, nres, base) };
    unsafe { libc::signal(libc::SIGINT, libc::SIG_DFL); } /* reset C-signal handler */
    unsafe { ffi::lua::lua_remove(l, base); }  /* remove message handler from the stack */
    status
}


fn print_version() {
    use std::io::{self, Write};
    writeln!(&mut io::stdout(), "{}", ffi::lua::LUA_COPYRIGHT).unwrap();
}


/*
** Create the 'arg' table, which stores all arguments from the
** command line ('options'). It should be aligned so that, at index 0,
** it has 'script_args[0]', which is the script name. The arguments
** to the script (everything after 'script_args[0]') go to positive indices;
** other arguments (pre_script_args) go to negative indices.
** If there is no script name, assume interpreter's name as base.
*/
fn createargtable<'a>(l: *mut ffi::lua::lua_State, options: &ProgramOptions<'a>) {
    let (pre, post) = if options.script_args.len() == 0 {  /* no script name? */
        (options.script_args, options.pre_script_args)
    } else {
        (options.pre_script_args, options.script_args)
    };
    unsafe { ffi::lua::lua_createtable(l, post.len() as i32, (pre.len() + 1) as i32); }
    for (i, arg) in pre.iter().enumerate() {
        let s = std::ffi::CString::new(*arg).unwrap();
        unsafe { ffi::lua::lua_pushstring(l, s.as_ptr()); }
        unsafe { ffi::lua::lua_rawseti(l, -2, i as i64 - pre.len() as i64); }
    }
    for (i, arg) in post.iter().enumerate() {
        let s = std::ffi::CString::new(*arg).unwrap();
        unsafe { ffi::lua::lua_pushstring(l, s.as_ptr()); }
        unsafe { ffi::lua::lua_rawseti(l, -2, i as i64); }
    }
    let s = std::ffi::CString::new("arg").unwrap();
    unsafe { ffi::lua::lua_setglobal(l, s.as_ptr()); }
}


fn dochunk(l: *mut ffi::lua::lua_State, status: libc::c_int) -> libc::c_int {
    let status = if status == ffi::lua::LUA_OK { docall(l, 0, 0) } else { status };
    report(l, status, true)
}


fn dofile(l: *mut ffi::lua::lua_State, name: Option<&str>) -> libc::c_int {
    let status = match name {
        Some(n) => {
            let s = std::ffi::CString::new(n).unwrap();
            unsafe { ffi::lauxlib::luaL_loadfile(l, s.as_ptr()) }
        }
        None => unsafe { ffi::lauxlib::luaL_loadfile(l, std::ptr::null()) },
    };
    dochunk(l, status)
}


fn dostring(l: *mut ffi::lua::lua_State, s: &str, name: &str) -> libc::c_int {
    let s = std::ffi::CString::new(s).unwrap();
    let name = std::ffi::CString::new(name).unwrap();
    dochunk(l, unsafe { ffi::lauxlib::luaL_loadbuffer(l, s.as_ptr(), s.to_bytes().len(), name.as_ptr()) })
}


/*
** Calls 'require(name)' and stores the result in a global variable
** with the given name.
*/
fn dolibrary(l: *mut ffi::lua::lua_State, name: &str) -> libc::c_int {
    let require = std::ffi::CString::new("require").unwrap();
    unsafe { ffi::lua::lua_getglobal(l, require.as_ptr()); }
    let name = std::ffi::CString::new(name).unwrap();
    unsafe { ffi::lua::lua_pushstring(l, name.as_ptr()); }
    let status = docall(l, 1, 1);  /* call 'require(name)' */
    if status == ffi::lua::LUA_OK {
        unsafe { ffi::lua::lua_setglobal(l, name.as_ptr()); }  /* global[name] = require return */
    }
    report(l, status, true)
}


/*
** Returns the string to be used as a prompt by the interpreter.
*/
fn get_prompt(l: *mut ffi::lua::lua_State, firstline: bool) -> String {
    let s = std::ffi::CString::new(if firstline { "_PROMPT" } else { "_PROMPT2" }).unwrap();
    unsafe { ffi::lua::lua_getglobal(l, s.as_ptr()); }
    let p = unsafe { ffi::lua::lua_tostring(l, -1) };
    if p.is_null() {
        if firstline { LUA_PROMPT } else { LUA_PROMPT2 }.to_string()
    } else {
        unsafe { std::ffi::CStr::from_ptr(p) }.to_str().unwrap().to_string()
    }
}

/* mark in error messages for incomplete statements */
const EOFMARK: &'static str = "<eof>";


/*
** Check whether 'status' signals a syntax error and the error
** message at the top of the stack ends with the above mark for
** incomplete statements.
*/
fn incomplete(l: *mut ffi::lua::lua_State, status: libc::c_int) -> bool {
    if status == ffi::lua::LUA_ERRSYNTAX {
        let mut lmsg: libc::size_t = 0;
        let msg = unsafe { ffi::lua::lua_tolstring(l, -1, &mut lmsg) };
        let s = unsafe { std::ffi::CStr::from_ptr(msg) };
        if s.to_str().unwrap().ends_with(EOFMARK) {
            unsafe { ffi::lua::lua_pop(l, 1); }
            return true;
        }
    }
    false  /* else... */
}


/*
** Prompt the user, read a line, and push it into the Lua stack.
*/
fn pushline(l: *mut ffi::lua::lua_State, firstline: bool) -> bool {
    let prompt = get_prompt(l, firstline);
    let mut line = match readline(&prompt) {
        Some(l) => l,
        None => return false,
    };
    if line.is_empty() {
        return false;  /* no input (prompt will be popped by caller) */
    }
    unsafe { ffi::lua::lua_pop(l, 1); }  /* remove prompt */
    if line.ends_with('\n') {  /* line ends with newline? */
        line.pop();  /* remove it */
    }
    let s = std::ffi::CString::new(line).unwrap();
    unsafe { ffi::lua::lua_pushlstring(l, s.as_ptr(), s.to_bytes().len()); }
    true
}


/*
** Try to compile line on the stack as 'return <line>;'; on return, stack
** has either compiled chunk or original line (if compilation failed).
*/
fn addreturn(l: *mut ffi::lua::lua_State) -> libc::c_int {
    let line = unsafe { ffi::lua::lua_tostring(l, -1) };  /* original line */
    let s = std::ffi::CString::new("return %s;").unwrap();
    let retline = unsafe { ffi::lua::lua_pushfstring(l, s.as_ptr(), line) };
    let s = std::ffi::CString::new("=stdin").unwrap();
    let status = unsafe { ffi::lauxlib::luaL_loadbuffer(l, retline, libc::strlen(retline), s.as_ptr()) };
    if status == ffi::lua::LUA_OK {
        unsafe { ffi::lua::lua_remove(l, -2); }  /* remove modified line */
        let s = unsafe { std::ffi::CStr::from_ptr(line) };
        if !s.to_bytes().is_empty() {  /* non empty? */
            saveline(s.to_str().unwrap());  /* keep history */
        }
    } else {
        unsafe { ffi::lua::lua_pop(l, 2); }  /* pop result from 'luaL_loadbuffer' and modified line */
    }
    status
}


/*
** Read multiple lines until a complete Lua statement
*/
fn multiline(l: *mut ffi::lua::lua_State) -> libc::c_int {
    loop {  /* repeat until gets a complete statement */
        let mut len: libc::size_t = 0;
        let line = unsafe { ffi::lua::lua_tolstring(l, 1, &mut len) };  /* get what it has */
        let s = std::ffi::CString::new("=stdin").unwrap();
        let status = unsafe { ffi::lauxlib::luaL_loadbuffer(l, line, len, s.as_ptr()) };  /* try it */
        if !incomplete(l, status) || !pushline(l, false) {
            let s = unsafe { std::ffi::CStr::from_ptr(line) };
            saveline(s.to_str().unwrap());  /* keep history */
            return status;  /* cannot or should not try to add continuation line */
        }
        unsafe { ffi::lua::lua_pushliteral(l, "\n"); }  /* add newline... */
        unsafe { ffi::lua::lua_insert(l, -2); }  /* ...between the two lines */
        unsafe { ffi::lua::lua_concat(l, 3); }  /* join them */
    }
}


/*
** Read a line and try to load (compile) it first as an expression (by
** adding "return " in front of it) and second as a statement. Return
** the final status of load/call with the resulting function (if any)
** in the top of the stack.
*/
fn loadline(l: *mut ffi::lua::lua_State) -> libc::c_int {
    unsafe { ffi::lua::lua_settop(l, 0); }
    if !pushline(l, true) {
        return -1;  /* no input */
    }
    let mut status = addreturn(l);
    if status != ffi::lua::LUA_OK {  /* 'return ...' did not work? */
        status = multiline(l);  /* try as command, maybe with continuation lines */
    }
    unsafe { ffi::lua::lua_remove(l, 1); }  /* remove line from the stack */
    assert!(unsafe { ffi::lua::lua_gettop(l) } == 1);
    status
}


/*
** Prints (calling the Lua 'print' function) any values on the stack
*/
fn print(l: *mut ffi::lua::lua_State) {
    let n = unsafe { ffi::lua::lua_gettop(l) };
    if n > 0 {  /* any result to be printed? */
        let s = std::ffi::CString::new("too many results to print").unwrap();
        unsafe { ffi::lauxlib::luaL_checkstack(l, ffi::lua::LUA_MINSTACK, s.as_ptr()); }
        let s = std::ffi::CString::new("print").unwrap();
        unsafe { ffi::lua::lua_getglobal(l, s.as_ptr()); }
        unsafe { ffi::lua::lua_insert(l, 1); }
        if unsafe { ffi::lua::lua_pcall(l, n, 0, 0) } != ffi::lua::LUA_OK {
            let s = std::ffi::CString::new("error calling 'print' (%s)").unwrap();
            let msg = unsafe { ffi::lua::lua_pushfstring(l, s.as_ptr(), ffi::lua::lua_tostring(l, -1)) };
            message(unsafe { std::ffi::CStr::from_ptr(msg) }.to_str().unwrap(), false);
        }
    }
}


/*
** Do the REPL: repeatedly read (load) a line, evaluate (call) it, and
** print any results.
*/
fn dorepl(l: *mut ffi::lua::lua_State) {
    loop {
        let mut status = loadline(l);
        if status == -1 { break; }
        if status == ffi::lua::LUA_OK {
            status = docall(l, 0, ffi::lua::LUA_MULTRET);
        }
        if status == ffi::lua::LUA_OK { print(l); }
        else { report(l, status, false); }  /* no 'progname' on errors in interactive mode */
    }
    unsafe { ffi::lua::lua_settop(l, 0); }  /* clear stack */
    use std::io::{self, Write};
    writeln!(io::stdout(), "").unwrap();
}


/*
** Push on the stack the contents of table 'arg' from 1 to #arg
*/
fn pushargs(l: *mut ffi::lua::lua_State) -> libc::c_int {
    let arg = std::ffi::CString::new("arg").unwrap();
    if unsafe { ffi::lua::lua_getglobal(l, arg.as_ptr()) } != ffi::lua::LUA_TTABLE {
        let s = std::ffi::CString::new("'arg' is not a table").unwrap();
        unsafe { ffi::lauxlib::luaL_error(l, s.as_ptr()); }
    }
    let n = unsafe { ffi::lauxlib::luaL_len(l, -1) } as i32;
    let s = std::ffi::CString::new("too many arguments to script").unwrap();
    unsafe { ffi::lauxlib::luaL_checkstack(l, n + 3, s.as_ptr()); }
    for i in 1..(n + 1) {
        unsafe { ffi::lua::lua_rawgeti(l, -i, i as i64); }
    }
    unsafe { ffi::lua::lua_remove(l, -(n + 1)); }  /* remove table from the stack */
    n
}


fn handle_script(l: *mut ffi::lua::lua_State, args: &[&str], stop_options: bool) -> libc::c_int {
    let s = std::ffi::CString::new(args[0]).unwrap();
    let fname = if args[0] == "-" && !stop_options {
        std::ptr::null()  /* stdin */
    } else {
        s.as_ptr()
    };
    let mut status = unsafe { ffi::lauxlib::luaL_loadfile(l, fname) };
    if status == ffi::lua::LUA_OK {
        let n = pushargs(l);  /* push arguments to script */
        status = docall(l, n, ffi::lua::LUA_MULTRET);
    }
    report(l, status, true)
}


#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
enum RunnableArg<'a> {
    Library(&'a str), /* -l */
    Execute(&'a str), /* -e */
}

/* represents the various argument indicators in 'args' */
#[derive(Default, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct ProgramOptions<'a> {
    interactive: bool,                   /* -i */
    version: bool,                       /* -v */
    ignore_env: bool,                    /* -E */
    execute: bool,                       /* -e */
    stop_options: bool,                  /* -- */
    runnable_args: Vec<RunnableArg<'a>>, /* -l or -e */
    pre_script_args: &'a [&'a str],      /* before script name */
    script_args: &'a [&'a str],          /* script name and args */
}


/*
** Traverses all arguments from 'args', returning a ProgramOptions with those
** needed before running any Lua code (or an error code if it finds
** any invalid argument). It returns the first not-handled argument as
** either the script name or a bad argument in case of error.
*/
fn collectargs<'a>(args: &'a [&str]) -> Result<ProgramOptions<'a>, &'a str> {
    fn add_script_args<'b>(mut options: ProgramOptions<'b>, args: &'b [&str], script: usize)
        -> Result<ProgramOptions<'b>, &'b str> {
        let (pre_script_args, script_args) = args.split_at(script);
        options.pre_script_args = pre_script_args;
        options.script_args = script_args;
        Ok(options)
    }
    use RunnableArg::{Library, Execute};
    let mut options: ProgramOptions = Default::default();
    let mut first = 0usize;
    let mut skip = 0;
    for (i, arg) in args.iter().enumerate().skip(1) {
        first = i;
        if skip != 0 {
            if arg.chars().next().unwrap() == '-' { /* another option instead of argument */
                return Err(args[first]);
            }
            options.runnable_args.push(if skip == 1 { Library(arg) } else { Execute(arg) });
            skip = 0;
            continue;
        }
        if arg.chars().next().unwrap() != '-' {  /* not an option? */
            return add_script_args(options, args, first);  /* stop handling options */
        }
        if arg.len() == 1 {  /* '-' */
            return add_script_args(options, args, first);  /* script "name" is '-' */
        }
        match arg.chars().skip(1).next().unwrap() {  /* else check option */
            '-' => {  /* '--' */
                if arg.len() != 2 {  /* extra characters after '--'? */
                    return Err(args[first]);  /* invalid option */
                }
                options.stop_options = true;
                return add_script_args(options, args, first + 1);
            }
            'E' => {
                if arg.len() != 2 {  /* extra characters after 1st? */
                    return Err(args[first]);  /* invalid option */
                }
                options.ignore_env = true;
            }
            'i' => {  /* (-i implies -v) */
                if arg.len() != 2 {  /* extra characters after 1st? */
                    return Err(args[first]);  /* invalid option */
                }
                options.interactive = true;
                options.version = true;
            }
            'v' => {
                if arg.len() != 2 {  /* extra characters after 1st? */
                    return Err(args[first]);  /* invalid option */
                }
                options.version = true;
            }
            'e' => {
                options.execute = true;
                if arg.len() == 2 {  /* no concatenated argument? */
                    skip = 2;  /* try next 'arg' */
                } else {
                    let (_, a) = arg.split_at(2);
                    options.runnable_args.push(Execute(a));
                }
            }
            'l' => {
                if arg.len() == 2 {  /* no concatenated argument? */
                    skip = 1;  /* try next 'arg' */
                } else {
                    let (_, a) = arg.split_at(2);
                    options.runnable_args.push(Library(a));
                }
            }
            _ => return Err(args[first]),  /* invalid option */
        }
    }
    if skip != 0 {  /* no argument to option */
        return Err(args[first])
    }
    add_script_args(options, args, first + 1)  /* no script name */
}


/*
** Processes options 'e' and 'l', which involve running Lua code.
** Returns 0 if some code raises an error.
*/
fn runargs<'a>(l: *mut ffi::lua::lua_State, runnable_args: &[RunnableArg<'a>]) -> bool {
    use RunnableArg::{Library, Execute};
    for arg in runnable_args {
        let status = match arg {
            &Library(s) => dolibrary(l, s),
            &Execute(s) => dostring(l, s, "=(command line)"),
        };
        if status != ffi::lua::LUA_OK { return false; }
    }
    true
}


fn handle_luainit(l: *mut ffi::lua::lua_State) -> libc::c_int {
    fn get_var(name: &str) -> Option<(String, &str)> {
        let mut chars = name.chars();
        chars.next();  /* skip the '=' to get the key */
        match std::env::var(chars.as_str()) {
            Ok(val) => Some((val, name)),
            Err(_) => None,
        }
    }
    let (init, name) = match get_var(LUA_INITVARVERSION_NAME).or_else(|| get_var(LUA_INIT_VAR_NAME)) {
        Some((i, n)) => (i, n),
        None => return ffi::lua::LUA_OK,
    };
    if init.starts_with("@") {
        let mut chars = init.chars();
        chars.next();  /* skip the '@' to get the key */
        dofile(l, Some(chars.as_str()))
    } else {
        dostring(l, &init, &name)
    }
}


#[cfg(debug_assertions)]
fn openlibs(l: *mut ffi::lua::lua_State) {
    unsafe { ffi::lualib::luaL_openlibs(l); }
    extern { fn luaB_opentests(l: *mut ffi::lua::lua_State) -> libc::c_int; }
    let s = std::ffi::CString::new("T").unwrap();
    unsafe { ffi::lauxlib::luaL_requiref(l, s.as_ptr(), Some(luaB_opentests), 1); }
}
#[cfg(not(debug_assertions))]
fn openlibs(l: *mut ffi::lua::lua_State) {
    unsafe { ffi::lualib::luaL_openlibs(l); }
}


/*
** Main body of stand-alone interpreter (to be called in protected mode).
** Reads the options and handles them all.
*/
fn pmain_(l: *mut ffi::lua::lua_State) -> libc::c_int {
    unsafe { ffi::lauxlib::luaL_checkversion(l); }  /* check that interpreter has correct version */
    let args: Vec<_> = std::env::args().collect();
    let arg_strs: Vec<_> = args.iter().map(|arg| arg.as_ref()).collect();
    let options = match collectargs(&arg_strs) {
        Ok(o) => o,
        Err(bad_arg) => {
            print_usage(bad_arg);
            return 0;
        }
    };
    if options.version {  /* option '-v'? */
        print_version();
    }
    if options.ignore_env {  /* option '-E'? */
        unsafe { ffi::lua::lua_pushboolean(l, 1); }  /* signal for libraries to ignore env. vars. */
        let noenv = std::ffi::CString::new("LUA_NOENV").unwrap();
        unsafe { ffi::lua::lua_setfield(l, ffi::lua::LUA_REGISTRYINDEX, noenv.as_ptr()); }
    }
    openlibs(l);  /* open standard libraries */
    createargtable(l, &options);  /* create table 'arg' */
    if !options.ignore_env {  /* no option '-E'? */
        if handle_luainit(l) != ffi::lua::LUA_OK {  /* run LUA_INIT */
            return 0;  /* error running LUA_INIT */
        }
    }
    if !runargs(l, &options.runnable_args) {  /* execute arguments -e and -l */
      return 0;  /* something failed */
    }
    if options.script_args.len() != 0 &&  /* execute main script (if there is one) */
            handle_script(l, options.script_args, options.stop_options) != ffi::lua::LUA_OK {
        return 0;
    }
    if options.interactive {  /* -i option? */
        dorepl(l);  /* do read-eval-print loop */
    } else if options.script_args.len() == 0 && !(options.execute || options.version) {  /* no arguments? */
        if stdin_is_tty() {  /* running in interactive mode? */
            print_version();
            dorepl(l);  /* do read-eval-print loop */
        } else {
            dofile(l, None);  /* executes stdin as a file */
        }
    }
    unsafe { ffi::lua::lua_pushboolean(l, 1); }  /* signal no errors */
    1
}

unsafe extern "C" fn pmain(l: *mut ffi::lua::lua_State) -> libc::c_int {
    pmain_(l)
}


#[cfg(debug_assertions)]
fn newstate() -> *mut ffi::lua::lua_State {
    #[repr(C)]
    struct Memcontrol {
      numblocks: libc::c_ulong,
      total: libc::c_ulong,
      maxmem: libc::c_ulong,
      memlimit: libc::c_ulong,
      objcount: [libc::c_ulong; ffi::lua::LUA_NUMTAGS as usize],
    }

    extern { static mut l_memcontrol: Memcontrol; }

    extern {
        fn debug_realloc(ud: *mut libc::c_void, ptr: *mut libc::c_void,
                         osize: libc::size_t, osize: libc::size_t) -> *mut libc::c_void;
    }
    unsafe { ffi::lua::lua_newstate(Some(debug_realloc), &mut l_memcontrol as *mut Memcontrol as *mut _) }
}
#[cfg(not(debug_assertions))]
fn newstate() -> *mut ffi::lua::lua_State {
    unsafe { ffi::lauxlib::luaL_newstate() }
}


fn main() {
    let l = newstate();  /* create state */
    if l.is_null() {
        message("cannot create state: not enough memory", true);
        std::process::exit(1);
    }
    unsafe { ffi::lua::lua_pushcfunction(l, Some(pmain)) };  /* to call 'pmain' in protected mode */
    let status = unsafe { ffi::lua::lua_pcall(l, 0, 1, 0) };  /* do the call */
    let result = unsafe { ffi::lua::lua_toboolean(l, -1) };  /* get result */
    report(l, status, true);
    unsafe { ffi::lua::lua_close(l) };
    std::process::exit(if result != 0 && status == ffi::lua::LUA_OK { 0 } else { 1 });
}

#[cfg(test)]
mod tests {
    use super::collectargs;
    use super::ProgramOptions;
    use super::RunnableArg::{Library, Execute};

    #[test]
    fn test_collectargs() {
        assert_eq!(collectargs(vec!("lua").as_slice()), Ok(ProgramOptions {
            pre_script_args: &["lua"], ..Default::default()
        }));
        assert_eq!(collectargs(vec!("lua", "name").as_slice()), Ok(ProgramOptions {
            pre_script_args: &["lua"], script_args: &["name"], ..Default::default()
        }));
        assert_eq!(collectargs(vec!("lua", "-").as_slice()), Ok(ProgramOptions {
            pre_script_args: &["lua"], script_args: &["-"], ..Default::default()
        }));
        assert_eq!(collectargs(vec!("lua", "--").as_slice()), Ok(ProgramOptions {
            stop_options: true, pre_script_args: &["lua", "--"], ..Default::default()
        }));
        assert_eq!(collectargs(vec!("lua", "-i").as_slice()), Ok(ProgramOptions {
            interactive: true, version: true, pre_script_args: &["lua", "-i"], ..Default::default()
        }));
        assert_eq!(collectargs(vec!("lua", "-v").as_slice()), Ok(ProgramOptions {
            version: true, pre_script_args: &["lua", "-v"], ..Default::default()
        }));
        assert_eq!(collectargs(vec!("lua", "-v", "name").as_slice()), Ok(ProgramOptions {
            version: true, pre_script_args: &["lua", "-v"], script_args: &["name"], ..Default::default()
        }));
        assert_eq!(collectargs(vec!("lua", "-e").as_slice()), Err("-e"));
        assert_eq!(collectargs(vec!("lua", "-escript").as_slice()), Ok(ProgramOptions {
            execute: true, runnable_args: vec!(Execute("script")), pre_script_args: &["lua", "-escript"],
            ..Default::default()
        }));
        assert_eq!(collectargs(vec!("lua", "-e", "script").as_slice()), Ok(ProgramOptions {
            execute: true, runnable_args: vec!(Execute("script")), pre_script_args: &["lua", "-e", "script"],
            ..Default::default()
        }));
        assert_eq!(collectargs(vec!("lua", "-E").as_slice()), Ok(ProgramOptions {
            ignore_env: true, pre_script_args: &["lua", "-E"], ..Default::default()
        }));
        assert_eq!(collectargs(vec!("lua", "-l").as_slice()), Err("-l"));
        assert_eq!(collectargs(vec!("lua", "-llib").as_slice()), Ok(ProgramOptions {
            runnable_args: vec!(Library("lib")), pre_script_args: &["lua", "-llib"], ..Default::default()
        }));
        assert_eq!(collectargs(vec!("lua", "-l", "lib").as_slice()), Ok(ProgramOptions {
            runnable_args: vec!(Library("lib")), pre_script_args: &["lua", "-l", "lib"], ..Default::default()
        }));
        assert_eq!(collectargs(vec!("lua", "-x").as_slice()), Err("-x"));
    }
}
