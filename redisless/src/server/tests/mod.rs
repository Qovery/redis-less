use crate::command::Command;
use crate::server::ServerState;
use crate::storage::in_memory::InMemoryStorage;
use crate::Server;
use redis::{Commands, Connection, RedisWrite, ToRedisArgs};
use rstest::*;
use std::fmt::{write, Debug, Display, Formatter, Result};
use CommandArg::{Int, Str};

fn get_server_connection(port: u16) -> (Server, Connection) {
    let server = Server::new(InMemoryStorage::new(), port);
    assert_eq!(server.start(), Some(ServerState::Started));

    let redis_client = redis::Client::open(format!("redis://127.0.0.1:{}/", port)).unwrap();
    (server, redis_client.get_connection().unwrap())
}

struct TestConnection {
    server: Server,
    con: Connection,
}

impl TestConnection {
    fn start(port: u16) -> Self {
        let (server, con) = get_server_connection(port);
        TestConnection { server, con }
    }
    fn redis_set(&mut self, k: CommandArg, v: CommandArg) {
        let _: () = self.con.set(k, v).unwrap();
    }
    fn redis_incr(&mut self, k: CommandArg, v: CommandArg) {
        let _: () = self.con.incr(k, v).unwrap();
    }
    fn redis_decr(&mut self, k: CommandArg, v: CommandArg) {
        let _: () = self.con.decr(k, v).unwrap();
    }
    fn test_redis_get(&mut self, k: CommandArg, v: CommandArg) {
        let res: String = self.con.get(k).unwrap();
        assert_eq!(res, v.to_string());
    }
    fn halt_running<S: ToString + Display>(&mut self, message: S) {
        self.stop();
        panic!("{}", message);
    }
    fn run(&mut self, case: Vec<Vec<CommandArg>>) {
        for mut defn in case {
            match defn.first() {
                None => self.halt_running("empty command definition"),
                Some(c) => match c {
                    Str("set") => {
                        if defn.len() != 3 {
                            self.halt_running(format!("wrong number of args {:?}", defn));
                        }
                        let (v, k) = (defn.pop(), defn.pop());
                        self.redis_set(k.unwrap(), v.unwrap());
                    }
                    Str("incr") => {
                        if defn.len() != 3 {
                            self.halt_running(format!("wrong number of args {:?}", defn));
                        }
                        let (v, k) = (defn.pop(), defn.pop());
                        self.redis_incr(k.unwrap(), v.unwrap());
                    }
                    Str("decr") => {
                        if defn.len() != 3 {
                            self.halt_running(format!("wrong number of args {:?}", defn));
                        }
                        let (v, k) = (defn.pop(), defn.pop());
                        self.redis_decr(k.unwrap(), v.unwrap());
                    }
                    Str("test_get") => {
                        if defn.len() != 3 {
                            self.halt_running(format!("wrong number of args {:?}", defn));
                        }
                        let (v, k) = (defn.pop(), defn.pop());
                        self.test_redis_get(k.unwrap(), v.unwrap());
                    }
                    _ => self.halt_running(format!("unrecognized command definition {:?}", defn)),
                },
            }
        }
        self.stop();
    }
    fn stop(&mut self) {
        assert_eq!(self.server.stop(), Some(ServerState::Stopped));
    }
}

#[derive(Clone, Copy, Debug)]
enum CommandArg<'a> {
    Str(&'a str),
    Int(i64),
}
struct StrData<'a>(&'a str);
struct IntData(i64);

impl<'a> CommandArg<'a> {
    fn str(self) -> Option<StrData<'a>> {
        if let CommandArg::Str(s) = self {
            Some(StrData(s))
        } else {
            None
        }
    }
    fn int(self) -> Option<IntData> {
        if let CommandArg::Int(n) = self {
            Some(IntData(n))
        } else {
            None
        }
    }
}

impl<'a> ToRedisArgs for CommandArg<'a> {
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + RedisWrite,
    {
        if let Some(inner) = self.int() {
            return out.write_arg_fmt(inner.0);
        }
        if let Some(inner) = self.str() {
            return out.write_arg(inner.0.as_bytes());
        }
        panic!("CommandArg variant unimplemented trait: ToRedisArgs");
    }
}

impl<'a> ToString for CommandArg<'a> {
    fn to_string(&self) -> String {
        if let Some(inner) = self.int() {
            return inner.0.to_string();
        }
        if let Some(inner) = self.str() {
            return inner.0.to_owned();
        }
        panic!("CommandArg variant unimplemented trait: ToString");
    }
}

impl From<i64> for CommandArg<'_> {
    fn from(n: i64) -> Self {
        CommandArg::Int(n)
    }
}

impl<'a> From<&'a str> for CommandArg<'a> {
    fn from(n: &'a str) -> Self {
        CommandArg::Str(n)
    }
}

#[macro_export]
macro_rules! command_args {
    ( $x0:expr $(, $x:expr )+ ) => {{
        let mut v: Vec<CommandArg> = Vec::new();
        v.push( $x0.into() );
        $(
            v.push( $x.into() );
        )*
        v
    }};
}

#[rstest]
#[case::incr_decr_by_1(
    3001,
    vec![
        command_args!["set", "some_number", "12"],
        command_args!["incr", "some_number", 1],
        command_args!["test_get", "some_number", 13],

        command_args!["set", "n", 100],
        command_args!["decr", "n", "1"],
        command_args!["test_get", "n", "99"],
    ]
)]
#[case::incr_decr_by_delta(
    3002,
    vec![
        command_args!["set", "0", 12],
        command_args!["incr", "0", 500],
        command_args!["test_get", "0", "512"],
        command_args!["incr", "0", -10],
        command_args!["test_get", "0", "502"],

        command_args!["set", "63", 89],
        command_args!["decr", "63", 10],
        command_args!["test_get", "63", "79"],
        command_args!["decr", "63", "-100"],
        command_args!["test_get", "63", "179"],
    ]
)]
#[case::set_existent_key(
    3003,
    vec![
        command_args!["set", 12, "5"],
        command_args!["set", "12", 1200],
        command_args!["test_get", 12, "1200"],
    ]
)]
fn test_redis_client(#[case] port: u16, #[case] commands: Vec<Vec<CommandArg>>) {
    let mut t = TestConnection::start(port);
    t.run(commands);
}
