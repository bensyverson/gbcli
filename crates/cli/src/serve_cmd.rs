//! `gbcli serve`: the JSON Lines server over this process's stdio.

use crate::build::Target;
use crate::{Env, Io, exec, render};

pub fn serve(target: &Target, env: &Env, mut io: Io) -> u8 {
    let mut handler = match exec::handler(target, env) {
        Ok(handler) => handler,
        Err(body) => return render::error(io.err, &body),
    };
    match gbcli_server::serve(&mut io.stdin, &mut io.out, &mut handler) {
        Ok(()) => 0,
        Err(e) => {
            exec::emit(io.err, &format!("gbcli serve: {e}\n"));
            5
        }
    }
}
