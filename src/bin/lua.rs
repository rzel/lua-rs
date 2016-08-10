// Copyright (C) 1994-2015 Lua.org, PUC-Rio.
// Copyright (C) 2016 Ahmed Charles - acharles@outlook.com
// Distributed under the MIT License.
//    (See accompanying file LICENSE.txt or copy at
//          http://opensource.org/licenses/MIT)

// Lua stand-alone interpreter

extern crate libc;
extern crate lua_rs;

use lua_rs::ffi;

// #define lua_c

// #include "lprefix.h"


// #include <signal.h>
// #include <stdio.h>
// #include <stdlib.h>
// #include <string.h>

// #include "lua.h"

// #include "lauxlib.h"
// #include "lualib.h"


// #if !defined(LUA_PROMPT)
// #define LUA_PROMPT		"> "
// #define LUA_PROMPT2		">> "
// #endif

// #if !defined(LUA_PROGNAME)
// #define LUA_PROGNAME		"lua"
// #endif

// #if !defined(LUA_MAXINPUT)
// #define LUA_MAXINPUT		512
// #endif

const LUA_INIT_VAR_NAME: &'static str = "=LUA_INIT";

const LUA_INITVARVERSION_NAME: &'static str = "=LUA_INIT_5_3";


/*
** lua_stdin_is_tty detects whether the standard input is a 'tty' (that
** is, whether we're running lua interactively).
*/
// #if !defined(lua_stdin_is_tty)	/* { */

// #if defined(LUA_USE_POSIX)	/* { */

// #include <unistd.h>
// #define lua_stdin_is_tty()	isatty(0)

// #elif defined(LUA_USE_WINDOWS)	/* }{ */

// #include <io.h>
// #define lua_stdin_is_tty()	_isatty(_fileno(stdin))

// #else				/* }{ */

/* ISO C definition */
// #define lua_stdin_is_tty()	1  /* assume stdin is a tty */

// #endif				/* } */

// #endif				/* } */


/*
** lua_readline defines how to show a prompt and then read a line from
** the standard input.
** lua_saveline defines how to "save" a read line in a "history".
** lua_freeline defines how to free a line read by lua_readline.
*/
// #if !defined(lua_readline)	/* { */

// #if defined(LUA_USE_READLINE)	/* { */

// #include <readline/readline.h>
// #include <readline/history.h>
// #define lua_readline(L,b,p)	((void)L, ((b)=readline(p)) != NULL)
// #define lua_saveline(L,line)	((void)L, add_history(line))
// #define lua_freeline(L,b)	((void)L, free(b))

// #else				/* }{ */

// #define lua_readline(L,b,p) \
//         ((void)L, fputs(p, stdout), fflush(stdout),  /* show prompt */ \
//         fgets(b, LUA_MAXINPUT, stdin) != NULL)  /* get line */
// #define lua_saveline(L,line)	{ (void)L; (void)line; }
// #define lua_freeline(L,b)	{ (void)L; (void)b; }

// #endif				/* } */

// #endif				/* } */




// static lua_State *globalL = NULL;

// static const char *progname = LUA_PROGNAME;


/*
** Hook set by signal function to stop the interpreter.
*/
// static void lstop (lua_State *L, lua_Debug *ar) {
//   (void)ar;  /* unused arg. */
//   lua_sethook(L, NULL, 0, 0);  /* reset hook */
//   luaL_error(L, "interrupted!");
// }


/*
** Function to be called at a C signal. Because a C signal cannot
** just change a Lua state (as there is no proper synchronization),
** this function only sets a hook that, when called, will stop the
** interpreter.
*/
// static void laction (int i) {
//   signal(i, SIG_DFL); /* if another SIGINT happens, terminate process */
//   lua_sethook(globalL, lstop, LUA_MASKCALL | LUA_MASKRET | LUA_MASKCOUNT, 1);
// }


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
//   globalL = L;  /* to be available to 'laction' */
//   signal(SIGINT, laction);  /* set C-signal handler */
    let status = unsafe { ffi::lua::lua_pcall(l, narg, nres, base) };
//   signal(SIGINT, SIG_DFL); /* reset C-signal handler */
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
// static const char *get_prompt (lua_State *L, int firstline) {
//   const char *p;
//   lua_getglobal(L, firstline ? "_PROMPT" : "_PROMPT2");
//   p = lua_tostring(L, -1);
//   if (p == NULL) p = (firstline ? LUA_PROMPT : LUA_PROMPT2);
//   return p;
// }

/* mark in error messages for incomplete statements */
// #define EOFMARK		"<eof>"
// #define marklen		(sizeof(EOFMARK)/sizeof(char) - 1)


/*
** Check whether 'status' signals a syntax error and the error
** message at the top of the stack ends with the above mark for
** incomplete statements.
*/
// static int incomplete (lua_State *L, int status) {
//   if (status == LUA_ERRSYNTAX) {
//     size_t lmsg;
//     const char *msg = lua_tolstring(L, -1, &lmsg);
//     if (lmsg >= marklen && strcmp(msg + lmsg - marklen, EOFMARK) == 0) {
//       lua_pop(L, 1);
//       return 1;
//     }
//   }
//   return 0;  /* else... */
// }


/*
** Prompt the user, read a line, and push it into the Lua stack.
*/
// static int pushline (lua_State *L, int firstline) {
//   char buffer[LUA_MAXINPUT];
//   char *b = buffer;
//   size_t l;
//   const char *prmt = get_prompt(L, firstline);
//   int readstatus = lua_readline(L, b, prmt);
//   if (readstatus == 0)
//     return 0;  /* no input (prompt will be popped by caller) */
//   lua_pop(L, 1);  /* remove prompt */
//   l = strlen(b);
//   if (l > 0 && b[l-1] == '\n')  /* line ends with newline? */
//     b[--l] = '\0';  /* remove it */
//   if (firstline && b[0] == '=')  /* for compatibility with 5.2, ... */
//     lua_pushfstring(L, "return %s", b + 1);  /* change '=' to 'return' */
//   else
//     lua_pushlstring(L, b, l);
//   lua_freeline(L, b);
//   return 1;
// }


/*
** Try to compile line on the stack as 'return <line>;'; on return, stack
** has either compiled chunk or original line (if compilation failed).
*/
// static int addreturn (lua_State *L) {
//   const char *line = lua_tostring(L, -1);  /* original line */
//   const char *retline = lua_pushfstring(L, "return %s;", line);
//   int status = luaL_loadbuffer(L, retline, strlen(retline), "=stdin");
//   if (status == LUA_OK) {
//     lua_remove(L, -2);  /* remove modified line */
//     if (line[0] != '\0')  /* non empty? */
//       lua_saveline(L, line);  /* keep history */
//   }
//   else
//     lua_pop(L, 2);  /* pop result from 'luaL_loadbuffer' and modified line */
//   return status;
// }


/*
** Read multiple lines until a complete Lua statement
*/
// static int multiline (lua_State *L) {
//   for (;;) {  /* repeat until gets a complete statement */
//     size_t len;
//     const char *line = lua_tolstring(L, 1, &len);  /* get what it has */
//     int status = luaL_loadbuffer(L, line, len, "=stdin");  /* try it */
//     if (!incomplete(L, status) || !pushline(L, 0)) {
//       lua_saveline(L, line);  /* keep history */
//       return status;  /* cannot or should not try to add continuation line */
//     }
//     lua_pushliteral(L, "\n");  /* add newline... */
//     lua_insert(L, -2);  /* ...between the two lines */
//     lua_concat(L, 3);  /* join them */
//   }
// }


/*
** Read a line and try to load (compile) it first as an expression (by
** adding "return " in front of it) and second as a statement. Return
** the final status of load/call with the resulting function (if any)
** in the top of the stack.
*/
// static int loadline (lua_State *L) {
//   int status;
//   lua_settop(L, 0);
//   if (!pushline(L, 1))
//     return -1;  /* no input */
//   if ((status = addreturn(L)) != LUA_OK)  /* 'return ...' did not work? */
//     status = multiline(L);  /* try as command, maybe with continuation lines */
//   lua_remove(L, 1);  /* remove line from the stack */
//   lua_assert(lua_gettop(L) == 1);
//   return status;
// }


/*
** Prints (calling the Lua 'print' function) any values on the stack
*/
// static void l_print (lua_State *L) {
//   int n = lua_gettop(L);
//   if (n > 0) {  /* any result to be printed? */
//     luaL_checkstack(L, LUA_MINSTACK, "too many results to print");
//     lua_getglobal(L, "print");
//     lua_insert(L, 1);
//     if (lua_pcall(L, n, 0, 0) != LUA_OK)
//       l_message(progname, lua_pushfstring(L, "error calling 'print' (%s)",
//                                              lua_tostring(L, -1)));
//   }
// }


/*
** Do the REPL: repeatedly read (load) a line, evaluate (call) it, and
** print any results.
*/
// static void doREPL (lua_State *L) {
//   int status;
//   const char *oldprogname = progname;
//   progname = NULL;  /* no 'progname' on errors in interactive mode */
//   while ((status = loadline(L)) != -1) {
//     if (status == LUA_OK)
//       status = docall(L, 0, LUA_MULTRET);
//     if (status == LUA_OK) l_print(L);
//     else report(L, status);
//   }
//   lua_settop(L, 0);  /* clear stack */
//   lua_writeline();
//   progname = oldprogname;
// }


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
    unsafe { ffi::lualib::luaL_openlibs(l); }  /* open standard libraries */
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
//   if (args & has_i)  /* -i option? */
//     doREPL(L);  /* do read-eval-print loop */
//   else if (script == argc && !(args & (has_e | has_v))) {  /* no arguments? */
//     if (lua_stdin_is_tty()) {  /* running in interactive mode? */
//       print_version();
//       doREPL(L);  /* do read-eval-print loop */
//     }
//     else dofile(L, NULL);  /* executes stdin as a file */
//   }
    unsafe { ffi::lua::lua_pushboolean(l, 1); }  /* signal no errors */
    1
}

unsafe extern "C" fn pmain(l: *mut ffi::lua::lua_State) -> libc::c_int {
    pmain_(l)
}


fn main() {
    let l = unsafe { ffi::lauxlib::luaL_newstate() };  /* create state */
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
