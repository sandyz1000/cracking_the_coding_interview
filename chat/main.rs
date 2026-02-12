#![recursion_limit = "1024"] // `error_chain!` can recurse deeply

extern crate colored;
#[macro_use]
extern crate log;

use clap::{Arg, Command};
use colored::Colorize;
use log::{Level, LevelFilter, Metadata, Record};
use std::io::Read;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;

use std::error::Error;
use std::fmt;

/// Simple error type for this application
#[derive(Debug)]
pub struct AppError(String);

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for AppError {}

/// This Writer struct holds the information on a writer-thread so that
/// main_writer is able to send messages to all writers.
pub struct Writer {
    sender: mpsc::Sender<String>,
    id: usize,
}

/// Contains the action that main_writer should execute.
pub enum Action {
    ToWriters(String, Writer),
    AddWriter(Writer),
    RmWriter(Writer),
}
impl PartialEq for Writer {
    fn eq(self: &Self, rhs: &Self) -> bool {
        self.id == rhs.id
    }
}

/// Allows us to use `error!()`, `info!()`...
struct OurLogger;
impl log::Log for OurLogger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }
    fn log(&self, rec: &Record) {
        if self.enabled(rec.metadata()) {
            match rec.level() {
                Level::Error => eprintln!("{} {}", "error:".red().bold(), rec.args()),
                Level::Warn => eprintln!("{} {}", "warn:".yellow().bold(), rec.args()),
                Level::Info => eprintln!("{} {}", "info:".yellow().bold(), rec.args()),
                Level::Debug => eprintln!("{} {}", "debug:".bright_black().bold(), rec.args()),
                Level::Trace => eprintln!("{} {}", "trace:".blue().bold(), rec.args()),
            }
        }
    }
    fn flush(&self) {}
}

fn run(args: clap::ArgMatches) -> Result<(), String> {
    match args.subcommand() {
        Some(("server", subarg)) => {
            let port = subarg
                .get_one::<String>("PORT")
                .map(|s| s.as_str())
                .unwrap();
            let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
                .map_err(|_| format!("port '{}' already used", port.yellow()))?;
            info!("listening started");

            let (reader_send, to_main_writer) = mpsc::channel();

            // The 'main_writer' is the one who takes input from one of
            // incoming connections (from one of the readers) and send them
            // along to the other connections (passing a message to all the
            // writers).
            let _main_writer = thread::spawn(move || -> Result<(), String> {
                let mut writers: Vec<Writer> = Vec::new();
                while let Some(act) = to_main_writer.recv().ok() {
                    match act {
                        Action::ToWriters(msg, from) => {
                            writers.iter().filter(|to| **to != from).for_each(|to| {
                                debug!("ask writer n°{} to send '{}'", to.id, msg.yellow());
                                to.sender
                                    .send(msg.clone()) // Idk why this clone is required
                                    .unwrap_or_else(|_err| error!("cannot send to n°{}", to.id))
                            })
                        }
                        Action::AddWriter(w) => writers.push(w),
                        Action::RmWriter(w) => {
                            writers.retain(|writer| writer != &w);
                        }
                    }
                }
                Ok(())
            });

            for (id, stream) in listener.incoming().enumerate() {
                // Each reader thread must be able to send its received message
                // to the main_writer. As it is a all-to-one communication to the
                // main_chan, we can reuse the same sender.
                let reader_send = reader_send.clone();

                // On the contrary, sending a message from the main_writer to all
                // the writer threads is a one-to-all communication. As it is not
                // provided by the std lib, we will create one channel per writer.
                let (writer_send, writer_recv) = mpsc::channel();

                // Tell the main_writer that we got a new writer he should know of.
                reader_send
                    .send(Action::AddWriter(Writer {
                        sender: writer_send.clone(),
                        id,
                    }))
                    .map_err(|_| {
                        "couldn't add writer to the main writer (wtf this err msg?)".to_string()
                    })?;

                info!("incoming connection n°{}", id);

                let mut writer: TcpStream = stream.unwrap();
                let reader: TcpStream = writer.try_clone().unwrap();

                // The writer for this incoming connection. He is responsible for
                // sending the messages given by main_writer to the connection.
                thread::spawn(move || -> Result<(), String> {
                    writeln!(writer, "server: connected as n°{}", id).map_err(|e| e.to_string())?;
                    loop {
                        let msg = writer_recv
                            .recv()
                            .map_err(|_| "writer errored".to_string())?;
                        writeln!(writer, "{}", msg)
                            .map_err(|e| format!("error writing to connection n°{}: {}", id, e))?;
                        debug!("writer n°{} emited '{}'", id, msg.yellow());
                    }
                });

                // The reader for this incoming connection. He receives the messages
                // from the connection and passes them to the main_writer.
                thread::spawn(move || -> Result<(), String> {
                    // Note: `Read::chars()` has been removed. See:
                    // https://github.com/rust-lang/rust/issues/27802#issuecomment-377537778
                    // I wanted a quick fix, so I used `::io::Read::read_to_string`
                    // but it does allow replacing wrong utf-8 code points with the
                    // replacement character (`�`). The good option would be to use
                    // the utf8 crate and mimic the `reader.chars()` behaviour.
                    //Replacement: reader.read_to_string(&mut buf);
                    let reader_buf = BufReader::new(reader);
                    for line in reader_buf.lines() {
                        match line {
                            Ok(l) => {
                                debug!("reader n°{} received '{}'", id, l.yellow());
                                let sender = writer_send.clone();
                                reader_send
                                    .send(Action::ToWriters(l, Writer { sender, id }))
                                    .map_err(|_| "".to_string())?;
                            }
                            Err(e) => {
                                error!(
                                    "reader n°{} received a line with a wrong utf8 seq '{}'",
                                    id, e
                                );
                            }
                        }
                    }
                    Ok(())
                });
            }
        }

        Some(("client", subarg)) => {
            let address = subarg
                .get_one::<String>("ADDRESS")
                .map(|s| s.as_str())
                .unwrap();
            let port = subarg
                .get_one::<String>("PORT")
                .map(|s| s.as_str())
                .unwrap();
            let reader = TcpStream::connect(format!("{}:{}", address, port)).map_err(|_| {
                format!(
                    "could not connect to {}:{}",
                    address.yellow(),
                    port.yellow()
                )
            })?;
            let mut writer = reader
                .try_clone()
                .map_err(|_| "impossibe to clone the TCP stream (i.e., the socket)".to_string())?;

            info!("you can start typing");
            // The writer.
            let thread_writer = thread::spawn(move || -> Result<(), String> {
                let stdin = std::io::stdin();
                for b in stdin.bytes() {
                    let b = b.map_err(|e| e.to_string())?;
                    writer
                        .write(&[b])
                        .map_err(|e| format!("failed to write byte: {}", e))?;
                }
                Ok(())
            });
            // The reader.
            thread::spawn(move || -> Result<(), String> {
                let reader_buf = BufReader::new(reader);
                for line in reader_buf.lines() {
                    match line {
                        Ok(l) => println!("{} {}", "remote:".blue().bold(), l),
                        Err(e) => {
                            error!(
                                "{} received a line with a wrong utf8 seq: '{}'",
                                "remote:".blue().bold(),
                                e
                            );
                        }
                    }
                }
                Ok(())
            });
            // We must wait for the writing thread to terminate; otherwise,
            // the program will quit immediately.
            thread_writer
                .join()
                .unwrap()
                .map_err(|_| "writing thread errored".to_string())?;
        }
        _ => panic!("tell the dev: 'clap' should have ensured a subcommand is given"),
    }
    Ok(())
}

fn main() {
    log::set_logger(&OurLogger).unwrap();
    log::set_max_level(LevelFilter::Trace);

    let args = Command::new("myprog")
        .author("Me, me@mail.com")
        .version("1.0.2")
        .about("Explains in brief what the program does")
        .subcommand(
            Command::new("server")
                .about("run as server")
                .arg(Arg::new("PORT").required(true).help("Port to listen on")),
        )
        .subcommand(
            Command::new("client")
                .about("run as client")
                .arg(Arg::new("ADDRESS").required(true).help("address to use"))
                .arg(Arg::new("PORT").required(true).help("Port")),
        )
        .after_help(
            "Longer explaination to appear after the options when 
            displaying the help information from --help or -h",
        )
        .get_matches();

    // Run and handle errors.
    if let Err(e) = run(args) {
        error!("{}", e);
        std::process::exit(1);
    }
}
